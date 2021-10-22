import { get } from "./backed";
import { IViewHome } from "/shared/interfaces/ViewHome";

const API = {
  views: {
    home: async (): Promise<IViewHome> => {
      return get<IViewHome>(`/api/views/home`);
    },
  },
};
export default API;
