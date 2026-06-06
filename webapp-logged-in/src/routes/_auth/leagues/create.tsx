import Button from '@mui/material/Button';
import CircularProgress from '@mui/material/CircularProgress';
import FormHelperText from '@mui/material/FormHelperText';
import Stack from '@mui/material/Stack';
import TextField from '@mui/material/TextField';
import Typography from '@mui/material/Typography';
import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { SubmitHandler, useForm } from 'react-hook-form';
import { useMutation } from 'urql';
import { graphql } from '@/generated';

export const Route = createFileRoute('/_auth/leagues/create')({
  component: CreateLeaguePage,
});

const createLeagueMutation = graphql(`
  mutation CreateLeague(
    $name: String!
    $teamName: String!
    $userNickname: String!
  ) {
    createLeague(
      leagueName: $name
      teamName: $teamName
      userNickname: $userNickname
    ) {
      id
      ...CreateLeagueTeam
    }
  }

  fragment CreateLeagueTeam on League {
    id
    name
    teams {
      id
      name
    }
  }
`);

interface CreateLeagueFormFields {
  name: string;
  teamName: string;
  userNickname: string;
}

function CreateLeaguePage() {
  const navigate = useNavigate();
  const {
    formState: { errors: formErrors, isSubmitting },
    handleSubmit,
    register,
  } = useForm<CreateLeagueFormFields>({ mode: 'onBlur' });
  const [{ fetching, error }, executeCreateLeagueMutation] =
    useMutation(createLeagueMutation);

  const onSubmit: SubmitHandler<CreateLeagueFormFields> = async (data) => {
    const response = await executeCreateLeagueMutation(data, {
      additionalTypenames: ['League'],
    });

    const createdLeague = response.data?.createLeague;
    if (createdLeague) {
      await navigate({ to: '/league' });
    }
  };

  return (
    <form onSubmit={handleSubmit(onSubmit)}>
      <Stack spacing={3}>
        <Typography variant="h1">Create a League</Typography>
        <div>
          <TextField
            autoComplete="off"
            error={!!formErrors.name}
            fullWidth
            label="League name"
            {...register('name', { required: true })}
            slotProps={{ inputLabel: { shrink: true } }}
          />
          {formErrors.name && <FormHelperText error>Required</FormHelperText>}
        </div>

        <div>
          <TextField
            autoComplete="off"
            error={!!formErrors.teamName}
            fullWidth
            label="Team name"
            {...register('teamName', { required: true })}
            slotProps={{ inputLabel: { shrink: true } }}
          />
          {formErrors.teamName && (
            <FormHelperText error>Required</FormHelperText>
          )}
        </div>

        <div>
          <TextField
            autoComplete="off"
            error={!!formErrors.userNickname}
            fullWidth
            label="User nickname"
            {...register('userNickname', { required: true })}
            slotProps={{ inputLabel: { shrink: true } }}
          />
          {formErrors.userNickname && (
            <FormHelperText error>Required</FormHelperText>
          )}
        </div>
        <Button
          type="submit"
          disabled={isSubmitting || fetching}
          variant="contained"
          startIcon={
            isSubmitting || fetching ? (
              <CircularProgress size="1em" sx={{ mr: 1 }} />
            ) : undefined
          }
        >
          Create league
        </Button>
        {error?.message && (
          <FormHelperText error>{error.message}</FormHelperText>
        )}
      </Stack>
    </form>
  );
}
