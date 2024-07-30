'use client';

import {
  CreateLeagueTeamFragment,
  GetUserLeaguesDocument,
  useCreateLeagueMutation,
} from '@/generated/graphql';
import { SubmitHandler, useForm } from 'react-hook-form';
import { gql } from '@apollo/client';
import { useRouter } from 'next/navigation';
import Button from '@mui/material/Button';
import CircularProgress from '@mui/material/CircularProgress';
import FormHelperText from '@mui/material/FormHelperText';
import Stack from '@mui/material/Stack';
import TextField from '@mui/material/TextField';
import Typography from '@mui/material/Typography';

gql`
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
`;

interface CreateLeagueFormFields {
  name: string;
  teamName: string;
  userNickname: string;
}

export default function CreateLeaguePage() {
  const router = useRouter();
  const {
    formState: { errors: formErrors, isSubmitting },
    handleSubmit,
    register,
  } = useForm<CreateLeagueFormFields>({ mode: 'onBlur' });
  const [createLeagueMutation, { loading, error }] = useCreateLeagueMutation();

  const onSubmit: SubmitHandler<CreateLeagueFormFields> = async (data) => {
    const response = await createLeagueMutation({
      variables: data,
      refetchQueries: [GetUserLeaguesDocument],
    });
    const createdLeague: CreateLeagueTeamFragment | undefined =
      response.data?.createLeague;
    // TODO: notification + redirect
    if (createdLeague) {
      router.push('/league');
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
            InputLabelProps={{ shrink: true }}
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
            InputLabelProps={{ shrink: true }}
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
            InputLabelProps={{ shrink: true }}
          />
          {formErrors.userNickname && (
            <FormHelperText error>Required</FormHelperText>
          )}
        </div>
        <Button
          type="submit"
          disabled={isSubmitting || loading}
          variant="contained"
          startIcon={
            isSubmitting || loading ? (
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
