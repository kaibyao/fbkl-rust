import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { Loader2 } from 'lucide-react';
import { SubmitHandler, useForm } from 'react-hook-form';
import { useMutation } from 'urql';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
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
    <div className="mx-auto w-full max-w-md px-6 py-10">
      <h1 className="mb-6 font-heading text-3xl font-black tracking-tight">
        Create a League
      </h1>

      <form onSubmit={handleSubmit(onSubmit)} className="flex flex-col gap-5">
        <div className="flex flex-col gap-1.5">
          <Label htmlFor="name">League name</Label>
          <Input
            id="name"
            autoComplete="off"
            aria-invalid={!!formErrors.name}
            {...register('name', { required: true })}
          />
          {formErrors.name && (
            <p className="text-xs text-destructive">Required</p>
          )}
        </div>

        <div className="flex flex-col gap-1.5">
          <Label htmlFor="teamName">Team name</Label>
          <Input
            id="teamName"
            autoComplete="off"
            aria-invalid={!!formErrors.teamName}
            {...register('teamName', { required: true })}
          />
          {formErrors.teamName && (
            <p className="text-xs text-destructive">Required</p>
          )}
        </div>

        <div className="flex flex-col gap-1.5">
          <Label htmlFor="userNickname">User nickname</Label>
          <Input
            id="userNickname"
            autoComplete="off"
            aria-invalid={!!formErrors.userNickname}
            {...register('userNickname', { required: true })}
          />
          {formErrors.userNickname && (
            <p className="text-xs text-destructive">Required</p>
          )}
        </div>

        <Button
          type="submit"
          size="lg"
          disabled={isSubmitting || fetching}
          className="w-full hover:bg-primary-hot"
        >
          {(isSubmitting || fetching) && <Loader2 className="animate-spin" />}
          Create league
        </Button>

        {error?.message && (
          <p className="text-xs text-destructive">{error.message}</p>
        )}
      </form>
    </div>
  );
}
