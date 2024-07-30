import { BrowserRouter, Route, Routes } from 'react-router-dom';
import { FunctionComponent } from 'react';
import { LeagueDraftRoute } from '@/src/routes/league/draft/LeagueDraftRoute';
import { LeagueDraftYearRoute } from '@/src/routes/league/draft/LeagueDraftYearRoute';
import { LeagueHome } from '@/src/routes/league/LeagueHome';
import { LeagueInviteRoute } from '@/src/routes/league/invite/LeagueInviteRoute';
import { LeaguePlayerRoute } from '@/src/routes/league/player/LeaguePlayerRoute';
import { LeagueTeamRoute } from '@/src/routes/league/roster/LeagueTeamRoute';
import { TradesRoute } from '@/src/routes/league/trades/TradesRoute';
import { TransactionsRoute } from '@/src/routes/league/transactions/TransactionsRoute';
import { UserRoute } from '@/src/routes/user/UserRoute';

export const AppRoutes: FunctionComponent = () => {
  return (
    <BrowserRouter>
      <Routes>
        {/* <Route path="/app/league" element={<LeagueRoute />}> */}
        <Route index element={<LeagueHome />} />
        <Route path="draft" element={<LeagueDraftRoute />}>
          <Route path=":endOfSeasonYear" element={<LeagueDraftYearRoute />} />
        </Route>
        <Route path="invite" element={<LeagueInviteRoute />} />
        <Route path="player/:playerId" element={<LeaguePlayerRoute />} />
        <Route path="team/:teamId" element={<LeagueTeamRoute />} />
        <Route path="trades" element={<TradesRoute />} />
        <Route path="transactions" element={<TransactionsRoute />} />
        {/* </Route> */}
        <Route path="/app/user" element={<UserRoute />} />
        <Route path="/app/*" element={<div>Not found placeholder</div>} />
      </Routes>
    </BrowserRouter>
  );
};
