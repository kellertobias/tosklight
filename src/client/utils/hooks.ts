import { useEffect } from "react";

export const useEffectAsync = (effect: () => Promise<void>, deps?: React.DependencyList) => {
  useEffect(() => {
    effect();
  }, deps);
};
