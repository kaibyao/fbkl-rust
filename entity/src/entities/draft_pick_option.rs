//! `SeaORM` Entity. Generated by sea-orm-codegen 0.10.2

use std::collections::HashMap;

use async_graphql::Enum;
use async_trait::async_trait;
use once_cell::sync::Lazy;
use sea_orm::{entity::prelude::*, ActiveValue};
use serde::{Deserialize, Serialize};

/// A Draft Pick Option is an additional clause that can be applied to a draft pick. They are first created in a trade proposal and become active when a trade is processed.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "draft_pick_option")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub clause: String,
    pub status: DraftPickOptionStatus,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

/// Because a draft pick option first exists as a trade idea and is not yet solidified/applied to a draft pick, the status field is used to describe this intermediary state.
#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    Enum,
    Eq,
    Hash,
    PartialEq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
)]
#[sea_orm(rs_type = "i16", db_type = "Integer")]
pub enum DraftPickOptionStatus {
    /// The default status. This means the option has been proposed in a trade, but the trade has not been accepted yet.
    #[default]
    #[sea_orm(num_value = 0)]
    Proposed,
    /// The trade that created this option has been accepted and this option currently applies to the referenced draft pick.
    #[sea_orm(num_value = 1)]
    Active,
    /// This draft pick option has been activated + used on the draft pick.
    #[sea_orm(num_value = 2)]
    Used,
    /// The trade did not go through and this option died.
    #[sea_orm(num_value = 3)]
    CancelledViaTradeRejection,
    /// The draft pick option has been cancelled by a `DraftPickOptionAmendment`.
    #[sea_orm(num_value = 4)]
    CancelledViaDraftPickOptionAmendment,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::draft_pick::Entity")]
    DraftPick,
    #[sea_orm(has_many = "super::trade_asset::Entity")]
    TradeAsset,
}

impl Related<super::draft_pick::Entity> for Entity {
    // The final relation is DraftPickOption -> DraftPickDraftPickOption -> DraftPick
    fn to() -> RelationDef {
        super::draft_pick_draft_pick_option::Relation::DraftPick.def()
    }

    fn via() -> Option<RelationDef> {
        // The original relation is DraftPickDraftPickOption -> DraftPickOption,
        // after `rev` it becomes DraftPickOption -> DraftPickDraftPickOption
        Some(
            super::draft_pick_draft_pick_option::Relation::DraftPickOption
                .def()
                .rev(),
        )
    }
}

impl Related<super::trade_asset::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TradeAsset.def()
    }
}

#[async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(self, db: &C, is_insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        validate_status(&self, db, is_insert).await?;

        Ok(self)
    }
}

async fn validate_status<C>(model: &ActiveModel, db: &C, is_insert: bool) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    if is_insert {
        return if model.status != ActiveValue::Set(DraftPickOptionStatus::Proposed) {
            Err(DbErr::Custom(
                "A new draft pick option must be in the `Proposed` status.".to_string(),
            ))
        } else {
            Ok(())
        };
    }

    static VALID_BEFORE_AND_AFTER_STATUSES: Lazy<
        HashMap<&DraftPickOptionStatus, Vec<&DraftPickOptionStatus>>,
    > = Lazy::new(|| {
        [
            (
                &DraftPickOptionStatus::Proposed,
                vec![
                    &DraftPickOptionStatus::Active,
                    &DraftPickOptionStatus::CancelledViaTradeRejection,
                ],
            ),
            (
                &DraftPickOptionStatus::Active,
                vec![
                    &DraftPickOptionStatus::Used,
                    &DraftPickOptionStatus::CancelledViaDraftPickOptionAmendment,
                ],
            ),
            (&DraftPickOptionStatus::Used, vec![]),
            (
                &DraftPickOptionStatus::CancelledViaDraftPickOptionAmendment,
                vec![],
            ),
            (&DraftPickOptionStatus::CancelledViaTradeRejection, vec![]),
        ]
        .into_iter()
        .collect()
    });

    let ActiveValue::Set(Value::BigInt(maybe_id)) = model.get(Column::Id) else {
        return Err(DbErr::Custom("Couldn't extract id value from draft pick option ActiveModel".to_string()))
    };
    let id = maybe_id.ok_or_else(|| {
        DbErr::Custom("Non-inserted draft pick option doesn't have an id?!".to_string())
    })?;

    let current_saved = Entity::find_by_id(id).one(db).await?.ok_or_else(|| {
        DbErr::Custom(format!(
            "Couldn't find currently-persisted model of non-inserted draft pick option (id = {})",
            id
        ))
    })?;
    let current_status = current_saved.status;
    let ActiveValue::Set(new_status) = model.status else {
        return Err(DbErr::Custom(format!("Couldn't extract status value from draft pick option ActiveModel (id = {})", id)))
    };

    let valid_new_statuses = VALID_BEFORE_AND_AFTER_STATUSES
        .get(&current_status)
        .unwrap_or_else(|| {
            panic!(
                "VALID_BEFORE_AND_AFTER_STATUSES should have a value for key: {:?}",
                current_status
            )
        });

    if valid_new_statuses.contains(&&new_status) {
        Ok(())
    } else {
        Err(DbErr::Custom(format!("Not allowed to update a draft pick option whose previous status was {:?} and is now {:?}", current_status, new_status)))
    }
}
