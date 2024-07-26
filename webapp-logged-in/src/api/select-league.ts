import axios, { AxiosResponse } from 'axios';

export const selectLeague = (leagueId: number): Promise<AxiosResponse<void>> =>
  axios.post('/api/select_league', { leagueId });
