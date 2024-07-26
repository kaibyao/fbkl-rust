import {
  Box,
  Button,
  CircularProgress,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormHelperText,
  TextField,
} from '@mui/material';
import {
  CreateLeagueTeamFragment,
  GetUserLeaguesDocument,
  useCreateLeagueMutation,
} from '@logged-in/generated/graphql';
import { FunctionComponent } from 'react';
import { SubmitHandler, useForm } from 'react-hook-form';
import { gql } from '@apollo/client';

interface CreateLeagueFormFields {
  name: string;
  teamName: string;
  userNickname: string;
}

interface Props {
  onClose?: () => unknown;
  onCreateDone(createdLeague: CreateLeagueTeamFragment): void;
}

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

export const CreateLeagueForm: FunctionComponent<Props> = ({
  onClose,
  onCreateDone,
}) => {
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
      onCreateDone(createdLeague);
    }
  };

  return (
    <Dialog fullWidth open onClose={onClose}>
      <DialogTitle>Create a league</DialogTitle>
      <form onSubmit={handleSubmit(onSubmit)}>
        <DialogContent sx={{ pt: 1 }}>
          <Box mt={1}>
            <TextField
              autoComplete="off"
              error={!!formErrors.name}
              fullWidth
              label="League name"
              {...register('name', { required: true })}
              InputLabelProps={{ shrink: true }}
            />
            {formErrors.name && <FormHelperText error>Required</FormHelperText>}
          </Box>

          <Box mt={3}>
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
          </Box>

          <Box mt={3}>
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
          </Box>
        </DialogContent>
        <DialogActions sx={{ m: 2 }}>
          <Button onClick={onClose}>Cancel</Button>
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
        </DialogActions>
      </form>
    </Dialog>
  );
};
