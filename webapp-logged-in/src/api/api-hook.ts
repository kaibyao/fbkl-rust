import { AxiosError, AxiosResponse } from 'axios';
import { useState } from 'react';

interface AsyncRequestAttributes<T> {
  data?: T;
  error?: unknown;
  loading: boolean;
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function useAsyncRequest<Data, Args = any>(
  req: (...args: Args[]) => Promise<AxiosResponse<Data>>,
): [(...args: Args[]) => Promise<void>, AsyncRequestAttributes<Data>] {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<AxiosError>();
  const [data, setData] = useState<Data>();

  const requestCaller = async (...args: Args[]) => {
    setData(undefined);
    setError(undefined);

    try {
      setLoading(true);
      const response = await req(...args);
      setData(response.data);
    } catch (e) {
      setError(e as AxiosError);
      throw e;
    } finally {
      setLoading(false);
    }
  };

  return [
    requestCaller,
    {
      data,
      error,
      loading,
    },
  ];
}
