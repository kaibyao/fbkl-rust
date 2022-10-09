import { BrowserRouter, Route, Routes } from "react-router-dom";
import { CreateLeagueRoute } from "@routes/leagues/CreateLeagueRoute";
import { FunctionComponent } from "react";
import { LeagueHome } from "@logged-in/src/routes/leagues/LeagueHome";
import { LeagueRoute } from "@logged-in/src/routes/leagues/LeagueRoute";
import { UserLeaguesRoute } from "@logged-in/src/routes/leagues/UserLeaguesRoute";

export const AppRoutes: FunctionComponent = () => {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/app" element={<UserLeaguesRoute />}>
          <Route path="create" element={<CreateLeagueRoute />} />
        </Route>
        <Route path="/app/league/:leagueId" element={<LeagueRoute />}>
          <Route index element={<LeagueHome />} />
        </Route>
        <Route path="/app/*" element={<div>Not found placeholder</div>} />
      </Routes>
    </BrowserRouter>
  );
};
