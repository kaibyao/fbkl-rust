import { BrowserRouter, Route, Routes } from "react-router-dom";
import { CreateLeagueRoute } from "@routes/leagues/CreateLeagueRoute";
import { FunctionComponent } from "react";
import { LeagueDraftRoute } from "@logged-in/src/routes/leagues/draft/LeagueDraftRoute";
import { LeagueDraftYearRoute } from "@logged-in/src/routes/leagues/draft/LeagueDraftYearRoute";
import { LeagueHome } from "@logged-in/src/routes/leagues/LeagueHome";
import { LeagueInviteRoute } from "@logged-in/src/routes/leagues/LeagueInviteRoute";
import { LeaguePlayerRoute } from "@logged-in/src/routes/leagues/player/LeaguePlayerRoute";
import { LeagueRoute } from "@logged-in/src/routes/leagues/LeagueRoute";
import { LeagueTeamRoute } from "@logged-in/src/routes/leagues/roster/LeagueTeamRoute";
import { TradesRoute } from "@logged-in/src/routes/leagues/trades/TradesRoute";
import { TransactionsRoute } from "@logged-in/src/routes/leagues/transactions/TransactionsRoute";
import { UserLeaguesRoute } from "@logged-in/src/routes/leagues/UserLeaguesRoute";
import { UserRoute } from "@logged-in/src/routes/user/UserRoute";

export const AppRoutes: FunctionComponent = () => {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/app" element={<UserLeaguesRoute />}>
          <Route path="create" element={<CreateLeagueRoute />} />
        </Route>
        <Route path="/app/league/:leagueId" element={<LeagueRoute />}>
          <Route index element={<LeagueHome />} />
          <Route path="draft" element={<LeagueDraftRoute />}>
            <Route path=":seasonEndYear" element={<LeagueDraftYearRoute />} />
          </Route>
          <Route path="invite" element={<LeagueInviteRoute />} />
          <Route path="player/:playerId" element={<LeaguePlayerRoute />} />
          <Route path="team/:teamId" element={<LeagueTeamRoute />} />
          <Route path="trades" element={<TradesRoute />} />
          <Route path="transactions" element={<TransactionsRoute />} />
        </Route>
        <Route path="/app/user" element={<UserRoute />} />
        <Route path="/app/*" element={<div>Not found placeholder</div>} />
      </Routes>
    </BrowserRouter>
  );
};
