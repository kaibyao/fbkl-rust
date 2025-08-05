'use client';

import React from 'react';
import Box from '@mui/material/Box';
import Button from '@mui/material/Button';
import Card from '@mui/material/Card';
import CardContent from '@mui/material/CardContent';
import Chip from '@mui/material/Chip';
import CircularProgress from '@mui/material/CircularProgress';
import Stack from '@mui/material/Stack';
import Typography from '@mui/material/Typography';
import CheckCircleIcon from '@mui/icons-material/CheckCircle';
import ErrorIcon from '@mui/icons-material/Error';
import PendingIcon from '@mui/icons-material/Pending';
import PlayArrowIcon from '@mui/icons-material/PlayArrow';
import ScheduleIcon from '@mui/icons-material/Schedule';

export interface DeadlineConfigStatus {
  hasConfiguration: boolean;
  isActivated: boolean;
  draftDeadlinesCount: number;
  activatedDeadlinesCount: number;
  processedDeadlinesCount: number;
  canEdit: boolean;
}

export interface Deadline {
  id: number;
  dateTime: string;
  kind: string;
  name: string;
  status: 'Draft' | 'Activated' | 'Processing' | 'Processed' | 'Error';
}

interface DeadlineStatusDisplayProps {
  status: DeadlineConfigStatus;
  deadlines?: Deadline[];
  onActivate?: () => Promise<void>;
  isActivating?: boolean;
  activationError?: string | null;
  isCommissioner?: boolean;
}

const getStatusColor = (status: string) => {
  switch (status) {
    case 'Draft':
      return 'default';
    case 'Activated':
      return 'primary';
    case 'Processing':
      return 'warning';
    case 'Processed':
      return 'success';
    case 'Error':
      return 'error';
    default:
      return 'default';
  }
};

const getStatusIcon = (status: string) => {
  switch (status) {
    case 'Draft':
      return <PendingIcon fontSize="small" />;
    case 'Activated':
      return <PlayArrowIcon fontSize="small" />;
    case 'Processing':
      return <ScheduleIcon fontSize="small" />;
    case 'Processed':
      return <CheckCircleIcon fontSize="small" />;
    case 'Error':
      return <ErrorIcon fontSize="small" />;
    default:
      return <PendingIcon fontSize="small" />;
  }
};

const formatDateTime = (dateTimeString: string) => {
  try {
    const date = new Date(dateTimeString);
    return date.toLocaleString();
  } catch {
    return dateTimeString;
  }
};

export const DeadlineStatusDisplay: React.FC<DeadlineStatusDisplayProps> = ({
  status,
  deadlines = [],
  onActivate,
  isActivating = false,
  activationError = null,
  isCommissioner = false,
}) => {
  if (!status.hasConfiguration) {
    return (
      <Card>
        <CardContent>
          <Typography variant="h6" gutterBottom>
            No Configuration Found
          </Typography>
          <Typography color="text.secondary">
            This league does not have deadline configuration set up yet.
          </Typography>
        </CardContent>
      </Card>
    );
  }

  const totalDeadlines = status.draftDeadlinesCount + status.activatedDeadlinesCount + status.processedDeadlinesCount;
  const canActivate = isCommissioner && !status.isActivated && status.draftDeadlinesCount > 0;

  return (
    <Stack spacing={3}>
      {/* Status Overview */}
      <Card>
        <CardContent>
          <Typography variant="h6" gutterBottom>
            Deadline Status Overview
          </Typography>
          
          <Stack direction="row" spacing={2} sx={{ mb: 2 }}>
            <Chip
              label={`${status.draftDeadlinesCount} Draft`}
              color="default"
              icon={<PendingIcon />}
            />
            <Chip
              label={`${status.activatedDeadlinesCount} Activated`}
              color="primary"
              icon={<PlayArrowIcon />}
            />
            <Chip
              label={`${status.processedDeadlinesCount} Processed`}
              color="success"
              icon={<CheckCircleIcon />}
            />
          </Stack>

          {/* Activation Controls */}
          {canActivate && (
            <Box sx={{ mt: 2 }}>
              <Typography variant="body2" color="text.secondary" sx={{ mb: 1 }}>
                Ready to activate deadlines? This will make them available for automatic processing.
              </Typography>
              <Button
                variant="contained"
                color="primary"
                onClick={onActivate}
                disabled={isActivating}
                startIcon={
                  isActivating ? (
                    <CircularProgress size="1em" />
                  ) : (
                    <PlayArrowIcon />
                  )
                }
              >
                {isActivating ? 'Activating...' : 'Activate Deadlines'}
              </Button>
              {activationError && (
                <Typography color="error" variant="body2" sx={{ mt: 1 }}>
                  Error: {activationError}
                </Typography>
              )}
            </Box>
          )}

          {status.isActivated && (
            <Typography color="success.main" variant="body2" sx={{ mt: 2 }}>
              ✓ Deadlines have been activated and are being processed automatically
            </Typography>
          )}
        </CardContent>
      </Card>

      {/* Deadlines List */}
      {deadlines.length > 0 && (
        <Card>
          <CardContent>
            <Typography variant="h6" gutterBottom>
              Deadline Schedule
            </Typography>
            <Stack spacing={2}>
              {deadlines
                .sort((a, b) => new Date(a.dateTime).getTime() - new Date(b.dateTime).getTime())
                .map((deadline) => (
                  <Box
                    key={deadline.id}
                    sx={{
                      display: 'flex',
                      justifyContent: 'space-between',
                      alignItems: 'center',
                      p: 2,
                      border: '1px solid',
                      borderColor: 'divider',
                      borderRadius: 1,
                    }}
                  >
                    <Box>
                      <Typography variant="subtitle2" gutterBottom>
                        {deadline.name}
                      </Typography>
                      <Typography variant="body2" color="text.secondary">
                        {formatDateTime(deadline.dateTime)}
                      </Typography>
                    </Box>
                    <Chip
                      label={deadline.status}
                      color={getStatusColor(deadline.status) as any}
                      icon={getStatusIcon(deadline.status)}
                      size="small"
                    />
                  </Box>
                ))}
            </Stack>
          </CardContent>
        </Card>
      )}

      {/* Help Text */}
      <Card>
        <CardContent>
          <Typography variant="h6" gutterBottom>
            Status Explanation
          </Typography>
          <Stack spacing={1}>
            <Typography variant="body2">
              <strong>Draft:</strong> Configuration saved but not yet activated
            </Typography>
            <Typography variant="body2">
              <strong>Activated:</strong> Ready for automatic processing when the deadline time arrives
            </Typography>
            <Typography variant="body2">
              <strong>Processing:</strong> Currently being processed by the system
            </Typography>
            <Typography variant="body2">
              <strong>Processed:</strong> Successfully completed
            </Typography>
            <Typography variant="body2">
              <strong>Error:</strong> Processing failed - contact administrator
            </Typography>
          </Stack>
        </CardContent>
      </Card>
    </Stack>
  );
};