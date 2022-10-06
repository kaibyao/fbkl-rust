import { CreateLeagueForm } from "@logged-in/src/components/forms/CreateLeague/CreateLeagueForm";
import { FunctionComponent } from "react";
import { useNavigate } from "react-router-dom";

export const CreateLeagueRoute: FunctionComponent = () => {
  const navigate = useNavigate();
  const navigateToLeaguesHome = () => navigate("/leagues");
  return (
    <CreateLeagueForm
      onClose={navigateToLeaguesHome}
      // onCreateDone={navigateToLeaguesHome}
    />
  );
};
