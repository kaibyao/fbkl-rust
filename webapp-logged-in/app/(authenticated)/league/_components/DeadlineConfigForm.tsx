'use client';

import React from 'react';
import { SubmitHandler, useForm } from 'react-hook-form';
import Button from '@mui/material/Button';
import CircularProgress from '@mui/material/CircularProgress';
import FormHelperText from '@mui/material/FormHelperText';
import Stack from '@mui/material/Stack';
import TextField from '@mui/material/TextField';
import Typography from '@mui/material/Typography';
import Box from '@mui/material/Box';

export interface DeadlineConfigFormFields {
  preseasonKeeperDeadline: string;
  veteranAuctionDaysAfterKeeperDeadlineDuration: number;
  faAuctionDaysDuration: number;
  finalRosterLockDeadlineDaysAfterRookieDraft: number;
  playoffsStartWeek: number;
}

interface DeadlineConfigFormProps {
  initialValues?: Partial<DeadlineConfigFormFields>;
  onSubmit: (data: DeadlineConfigFormFields) => Promise<void>;
  isSubmitting?: boolean;
  error?: string | null;
  canEdit?: boolean;
}

const DEFAULT_VALUES: DeadlineConfigFormFields = {
  preseasonKeeperDeadline: '',
  veteranAuctionDaysAfterKeeperDeadlineDuration: 7,
  faAuctionDaysDuration: 7,
  finalRosterLockDeadlineDaysAfterRookieDraft: 14,
  playoffsStartWeek: 21,
};

export const DeadlineConfigForm: React.FC<DeadlineConfigFormProps> = ({
  initialValues = {},
  onSubmit,
  isSubmitting = false,
  error = null,
  canEdit = true,
}) => {
  const {
    formState: { errors: formErrors },
    handleSubmit,
    register,
  } = useForm<DeadlineConfigFormFields>({
    mode: 'onBlur',
    defaultValues: { ...DEFAULT_VALUES, ...initialValues },
  });

  const onSubmitHandler: SubmitHandler<DeadlineConfigFormFields> = async (data) => {
    await onSubmit(data);
  };

  const validateDateTime = (value: string) => {
    if (!value) return 'Required';
    try {
      const date = new Date(value);
      if (isNaN(date.getTime())) return 'Invalid date format';
      if (date <= new Date()) return 'Must be a future date';
      return true;
    } catch {
      return 'Invalid date format';
    }
  };

  const validatePositiveInteger = (value: number) => {
    if (!value || value <= 0) return 'Must be a positive number';
    if (!Number.isInteger(value)) return 'Must be a whole number';
    return true;
  };

  const validateWeek = (value: number) => {
    if (!value || value <= 0) return 'Must be a positive number';
    if (!Number.isInteger(value)) return 'Must be a whole number';
    if (value < 1 || value > 30) return 'Week must be between 1 and 30';
    return true;
  };

  return (
    <Box>
      <Typography variant="h3" gutterBottom>
        Deadline Configuration
      </Typography>
      <Typography variant="body1" color="text.secondary" paragraph>
        Configure the key deadlines for this league season. All deadline times
        are automatically calculated based on your configuration.
      </Typography>

      <form onSubmit={handleSubmit(onSubmitHandler)}>
        <Stack spacing={3}>
          {/* Preseason Keeper Deadline */}
          <div>
            <TextField
              fullWidth
              label="Preseason Keeper Deadline"
              type="datetime-local"
              disabled={!canEdit}
              error={!!formErrors.preseasonKeeperDeadline}
              helperText="The absolute deadline when keepers must be submitted"
              {...register('preseasonKeeperDeadline', {
                required: 'Required',
                validate: validateDateTime,
              })}
              InputLabelProps={{ shrink: true }}
            />
            {formErrors.preseasonKeeperDeadline && (
              <FormHelperText error>
                {formErrors.preseasonKeeperDeadline.message}
              </FormHelperText>
            )}
          </div>

          {/* Veteran Auction Start */}
          <div>
            <TextField
              fullWidth
              label="Days After Keeper Deadline for Veteran Auction Start"
              type="number"
              disabled={!canEdit}
              error={!!formErrors.veteranAuctionDaysAfterKeeperDeadlineDuration}
              helperText="Number of days after keeper deadline when veteran auction starts at 9 AM"
              {...register('veteranAuctionDaysAfterKeeperDeadlineDuration', {
                required: 'Required',
                valueAsNumber: true,
                validate: validatePositiveInteger,
              })}
              InputLabelProps={{ shrink: true }}
              inputProps={{ min: 1, step: 1 }}
            />
            {formErrors.veteranAuctionDaysAfterKeeperDeadlineDuration && (
              <FormHelperText error>
                {
                  formErrors.veteranAuctionDaysAfterKeeperDeadlineDuration
                    .message
                }
              </FormHelperText>
            )}
          </div>

          {/* FA Auction Duration */}
          <div>
            <TextField
              fullWidth
              label="Free Agent Auction Duration (Days)"
              type="number"
              disabled={!canEdit}
              error={!!formErrors.faAuctionDaysDuration}
              helperText="Number of days the free agent auction remains open"
              {...register('faAuctionDaysDuration', {
                required: 'Required',
                valueAsNumber: true,
                validate: validatePositiveInteger,
              })}
              InputLabelProps={{ shrink: true }}
              inputProps={{ min: 1, step: 1 }}
            />
            {formErrors.faAuctionDaysDuration && (
              <FormHelperText error>
                {formErrors.faAuctionDaysDuration.message}
              </FormHelperText>
            )}
          </div>

          {/* Final Roster Lock */}
          <div>
            <TextField
              fullWidth
              label="Days After Rookie Draft for Final Roster Lock"
              type="number"
              disabled={!canEdit}
              error={!!formErrors.finalRosterLockDeadlineDaysAfterRookieDraft}
              helperText="Number of days after rookie draft ends when final roster lock occurs"
              {...register('finalRosterLockDeadlineDaysAfterRookieDraft', {
                required: 'Required',
                valueAsNumber: true,
                validate: validatePositiveInteger,
              })}
              InputLabelProps={{ shrink: true }}
              inputProps={{ min: 1, step: 1 }}
            />
            {formErrors.finalRosterLockDeadlineDaysAfterRookieDraft && (
              <FormHelperText error>
                {formErrors.finalRosterLockDeadlineDaysAfterRookieDraft.message}
              </FormHelperText>
            )}
          </div>

          {/* Playoffs Start Week */}
          <div>
            <TextField
              fullWidth
              label="Playoffs Start Week"
              type="number"
              disabled={!canEdit}
              error={!!formErrors.playoffsStartWeek}
              helperText="Week number when playoffs begin (typically week 21)"
              {...register('playoffsStartWeek', {
                required: 'Required',
                valueAsNumber: true,
                validate: validateWeek,
              })}
              InputLabelProps={{ shrink: true }}
              inputProps={{ min: 1, max: 30, step: 1 }}
            />
            {formErrors.playoffsStartWeek && (
              <FormHelperText error>
                {formErrors.playoffsStartWeek.message}
              </FormHelperText>
            )}
          </div>

          {canEdit && (
            <Button
              type="submit"
              disabled={isSubmitting}
              variant="contained"
              size="large"
              startIcon={
                isSubmitting ? (
                  <CircularProgress size="1em" sx={{ mr: 1 }} />
                ) : undefined
              }
            >
              {initialValues?.preseasonKeeperDeadline
                ? 'Update Configuration'
                : 'Save Configuration'}
            </Button>
          )}

          {error && <FormHelperText error>{error}</FormHelperText>}

          {!canEdit && (
            <Typography color="text.secondary" variant="body2">
              Configuration cannot be modified after deadlines have been
              activated.
            </Typography>
          )}
        </Stack>
      </form>
    </Box>
  );
};
