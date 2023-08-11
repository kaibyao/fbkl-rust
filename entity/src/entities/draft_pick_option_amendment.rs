//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.2

use async_graphql::Enum;
use async_trait::async_trait;
use sea_orm::{entity::prelude::*, ActiveValue};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "draft_pick_option_amendment")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub draft_pick_option_id: i64,
    pub amended_clause: Option<String>,
    pub amendment_type: DraftPickOptionAmendmentType,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

/// Represents the different types of amendments that can be applied to a Draft Pick Option.
#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    Enum,
    Eq,
    PartialEq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
)]
#[sea_orm(rs_type = "i16", db_type = "Integer")]
pub enum DraftPickOptionAmendmentType {
    /// Cancel/nullify a draft pick option. This causes a draft pick option to no longer be applicable to its targeted draft picks.
    #[default]
    #[sea_orm(num_value = 0)]
    Cancellation,
    /// Amend the clause of a draft pick option. This changes the nature of the draft pick option's effects.
    #[sea_orm(num_value = 1)]
    ClauseAmendment,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::draft_pick_option::Entity",
        from = "Column::DraftPickOptionId",
        to = "super::draft_pick_option::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    DraftPickOption,
    #[sea_orm(has_one = "super::trade_asset::Entity")]
    TradeAsset,
}

impl Related<super::draft_pick_option::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::DraftPickOption.def()
    }
}

impl Related<super::trade_asset::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TradeAsset.def()
    }
}

#[async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(self, _db: &C, is_insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        only_insert_allowed(is_insert)?;
        validate_clause_required(&self)?;

        Ok(self)
    }
}

fn only_insert_allowed(is_insert: bool) -> Result<(), DbErr> {
    if !is_insert {
        Err(DbErr::Custom(
            "A draft pick option amendment cannot be updated; only inserted.".to_string(),
        ))
    } else {
        Ok(())
    }
}

fn validate_clause_required(model: &ActiveModel) -> Result<(), DbErr> {
    match model.amendment_type {
        ActiveValue::Set(DraftPickOptionAmendmentType::ClauseAmendment) => match &model.amended_clause {
            ActiveValue::Set(maybe_clause_text) => match maybe_clause_text {
                Some(clause_text) => match clause_text.trim() {
                    "" => Err(DbErr::Custom(
                        "A draft pick option amendment that amends an option’s clause must have actual text in its own clause.".to_string(),
                    )),
                    _ => Ok(())
                },
                None => Err(DbErr::Custom(
                    "A draft pick option amendment that amends an option’s clause must have its own clause to apply.".to_string(),
                )),
            },
            ActiveValue::NotSet => Err(DbErr::Custom(
                "A draft pick option amendment that amends an option’s clause must have its own clause to apply.".to_string(),
            )),
            _ => Ok(())
        },
        _ => Ok(())
    }
}
