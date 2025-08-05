'use client';

import React, { useCallback, useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';
import { useClient } from 'urql';
import Box from '@mui/material/Box';
import Stack from '@mui/material/Stack';
import Typography from '@mui/material/Typography';
import Alert from '@mui/material/Alert';
import CircularProgress from '@mui/material/CircularProgress';

import { useUserData } from '@/app/(authenticated)/league/_components/UserDataProvider';
import {
  DeadlineConfigForm,
  DeadlineConfigFormFields,
} from '@/app/(authenticated)/league/_components/DeadlineConfigForm';
import {
  DeadlineStatusDisplay,
  DeadlineConfigStatus,
  Deadline,
} from '@/app/(authenticated)/league/_components/DeadlineStatusDisplay';

// Manual GraphQL operations since schema generation is having issues
const CONFIGURE_SEASON_DEADLINES = `
  mutation ConfigureSeasonDeadlines($leagueId: Int!, $endOfSeasonYear: Int!, $config: DeadlineConfigRulesInput!) {
    configureSeasonDeadlines(leagueId: $leagueId, endOfSeasonYear: $endOfSeasonYear, config: $config) {
      id
      leagueId
      endOfSeasonYear
      preseasonKeeperDeadline
      veteranAuctionDaysAfterKeeperDeadlineDuration
      faAuctionDaysDuration
      finalRosterLockDeadlineDaysAfterRookieDraft
      playoffsStartWeek
    }
  }
`;

const ACTIVATE_SEASON_DEADLINES = `
  mutation ActivateSeasonDeadlines($leagueId: Int!, $endOfSeasonYear: Int!) {
    activateSeasonDeadlines(leagueId: $leagueId, endOfSeasonYear: $endOfSeasonYear)
  }
`;

const GET_DEADLINE_CONFIG_RULES = `
  query GetDeadlineConfigRules($leagueId: Int!, $endOfSeasonYear: Int!) {
    deadlineConfigRules(leagueId: $leagueId, endOfSeasonYear: $endOfSeasonYear) {
      id
      leagueId
      endOfSeasonYear
      preseasonKeeperDeadline
      veteranAuctionDaysAfterKeeperDeadlineDuration
      faAuctionDaysDuration
      finalRosterLockDeadlineDaysAfterRookieDraft
      playoffsStartWeek
    }
  }
`;

const GET_DEADLINE_CONFIG_STATUS = `
  query GetDeadlineConfigStatus($leagueId: Int!, $endOfSeasonYear: Int!) {
    deadlineConfigStatus(leagueId: $leagueId, endOfSeasonYear: $endOfSeasonYear) {
      hasConfiguration
      isActivated
      draftDeadlinesCount
      activatedDeadlinesCount
      processedDeadlinesCount
      canEdit
    }
  }
`;

const GET_DEADLINES = `
  query GetDeadlines($leagueId: Int!, $endOfSeasonYear: Int!) {
    deadlines(leagueId: $leagueId, endOfSeasonYear: $endOfSeasonYear) {
      id
      dateTime
      kind
      name
      endOfSeasonYear
      leagueId
      status
    }
  }
`;

// Helper function to handle GraphQL result errors
const handleGraphQLResult = (result: any) => {
  if (result.error) {
    throw new Error(result.error.message || 'GraphQL error');
  }
  return result.data;
};

export default function DeadlineConfigPage() {
  const userData = useUserData();
  const client = useClient();

  // Use current year + 1 as default end of season year (e.g., 2024-2025 season = 2025)
  const currentEndOfSeasonYear = new Date().getFullYear() + 1;

  const [status, setStatus] = useState<DeadlineConfigStatus | null>(null);
  const [deadlines, setDeadlines] = useState<Deadline[]>([]);
  const [configRules, setConfigRules] =
    useState<DeadlineConfigFormFields | null>(null);
  const [loading, setLoading] = useState(true);
  const [submitting, setSubmitting] = useState(false);
  const [activating, setActivating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [activationError, setActivationError] = useState<string | null>(null);

  const leagueId = userData.selectedLeagueId;

  // We'll determine commissioner status from the API response (canEdit field)
  const [isCommissioner, setIsCommissioner] = useState<boolean>(false);

  // Load data on component mount
  const loadData = useCallback(async () => {
    if (!leagueId) return;

    setLoading(true);
    setError(null);

    try {
      // Load status first
      const statusResult = await client
        .query(GET_DEADLINE_CONFIG_STATUS, {
          leagueId,
          endOfSeasonYear: currentEndOfSeasonYear,
        })
        .toPromise();

      const statusData = handleGraphQLResult(statusResult);
      setStatus(statusData.deadlineConfigStatus);

      // Set commissioner status based on whether user can edit
      setIsCommissioner(
        statusData.deadlineConfigStatus.canEdit ||
          !statusData.deadlineConfigStatus.hasConfiguration,
      );

      // If configuration exists, load the rules
      if (statusData.deadlineConfigStatus.hasConfiguration) {
        const rulesResult = await client
          .query(GET_DEADLINE_CONFIG_RULES, {
            leagueId,
            endOfSeasonYear: currentEndOfSeasonYear,
          })
          .toPromise();

        const rulesData = handleGraphQLResult(rulesResult);

        if (rulesData.deadlineConfigRules) {
          setConfigRules({
            preseasonKeeperDeadline:
              rulesData.deadlineConfigRules.preseasonKeeperDeadline.slice(
                0,
                16,
              ), // Format for datetime-local input
            veteranAuctionDaysAfterKeeperDeadlineDuration:
              rulesData.deadlineConfigRules
                .veteranAuctionDaysAfterKeeperDeadlineDuration,
            faAuctionDaysDuration:
              rulesData.deadlineConfigRules.faAuctionDaysDuration,
            finalRosterLockDeadlineDaysAfterRookieDraft:
              rulesData.deadlineConfigRules
                .finalRosterLockDeadlineDaysAfterRookieDraft,
            playoffsStartWeek: rulesData.deadlineConfigRules.playoffsStartWeek,
          });
        }

        // Load deadlines
        try {
          const deadlinesResult = await client
            .query(GET_DEADLINES, {
              leagueId,
              endOfSeasonYear: currentEndOfSeasonYear,
            })
            .toPromise();

          const deadlinesData = handleGraphQLResult(deadlinesResult);
          setDeadlines(deadlinesData.deadlines || []);
        } catch (deadlinesError) {
          console.warn('Could not load deadlines:', deadlinesError);
          // Don't set error state for deadlines since they might not exist yet
        }
      }
    } catch (err) {
      console.error('Error loading deadline configuration:', err);
      setError(
        err instanceof Error ? err.message : 'Failed to load configuration',
      );
    } finally {
      setLoading(false);
    }
  }, [leagueId, currentEndOfSeasonYear, client]);

  useEffect(() => {
    loadData();
  }, [loadData]);

  // Handle form submission
  const handleSubmit = async (data: DeadlineConfigFormFields) => {
    if (!leagueId) return;

    setSubmitting(true);
    setError(null);

    try {
      const result = await client
        .mutation(CONFIGURE_SEASON_DEADLINES, {
          leagueId,
          endOfSeasonYear: currentEndOfSeasonYear,
          config: data,
        })
        .toPromise();

      handleGraphQLResult(result);

      // Reload data after successful submission
      await loadData();
    } catch (err) {
      console.error('Error saving configuration:', err);
      setError(
        err instanceof Error ? err.message : 'Failed to save configuration',
      );
    } finally {
      setSubmitting(false);
    }
  };

  // Handle deadline activation
  const handleActivate = async () => {
    if (!leagueId) return;

    setActivating(true);
    setActivationError(null);

    try {
      const result = await client
        .mutation(ACTIVATE_SEASON_DEADLINES, {
          leagueId,
          endOfSeasonYear: currentEndOfSeasonYear,
        })
        .toPromise();

      handleGraphQLResult(result);

      // Reload data after successful activation
      await loadData();
    } catch (err) {
      console.error('Error activating deadlines:', err);
      setActivationError(
        err instanceof Error ? err.message : 'Failed to activate deadlines',
      );
    } finally {
      setActivating(false);
    }
  };

  // Redirect non-commissioners
  if (!isCommissioner) {
    return (
      <Box>
        <Alert severity="error">
          <Typography variant="h6">Access Denied</Typography>
          <Typography>
            Only league commissioners can access deadline configuration.
          </Typography>
        </Alert>
      </Box>
    );
  }

  if (loading) {
    return (
      <Box
        display="flex"
        justifyContent="center"
        alignItems="center"
        minHeight={200}
      >
        <CircularProgress />
      </Box>
    );
  }

  return (
    <Box>
      <Typography variant="body1" color="text.secondary" paragraph>
        Season {currentEndOfSeasonYear - 1}-{currentEndOfSeasonYear}
      </Typography>

      <Stack spacing={4}>
        {/* Status Display */}
        {status && (
          <DeadlineStatusDisplay
            status={status}
            deadlines={deadlines}
            onActivate={handleActivate}
            isActivating={activating}
            activationError={activationError}
            isCommissioner={isCommissioner}
          />
        )}

        {/* Configuration Form */}
        {(!status || status.canEdit) && (
          <DeadlineConfigForm
            initialValues={configRules || undefined}
            onSubmit={handleSubmit}
            isSubmitting={submitting}
            error={error}
            canEdit={!status || status.canEdit}
          />
        )}

        {/* General Error Display */}
        {error && (
          <Alert severity="error">
            <Typography variant="h6">Configuration Error</Typography>
            <Typography>{error}</Typography>
          </Alert>
        )}
      </Stack>
    </Box>
  );
}
