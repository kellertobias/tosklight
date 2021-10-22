import axios from "axios";

export const get = async <T>(path: string): Promise<T> => {
  return axios.get(path).then((res) => res.data as T);
};
