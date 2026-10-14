import { createContext, useCallback, useContext, useEffect, useState } from "react";

type Theme = "light" | "dark" | "system";

const Ctx = createContext<{ theme: Theme; effective: "light" | "dark"; setTheme: (t: Theme) => void }>({
  theme: "system",
  effective: "light",
  setTheme: () => {},
});

function resolve(theme: Theme): "light" | "dark" {
  if (theme !== "system") return theme;
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const [theme, setTheme] = useState<Theme>(() => (localStorage.getItem("liber-theme") as Theme) || "system");
  const [effective, setEffective] = useState<"light" | "dark">(() => resolve(theme));

  useEffect(() => {
    const next = resolve(theme);
    setEffective(next);
    document.documentElement.classList.toggle("dark", next === "dark");
    localStorage.setItem("liber-theme", theme);
  }, [theme]);

  useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => theme === "system" && setEffective(mq.matches ? "dark" : "light");
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, [theme]);

  const set = useCallback((t: Theme) => setTheme(t), []);
  return <Ctx.Provider value={{ theme, effective, setTheme: set }}>{children}</Ctx.Provider>;
}

export function useTheme() {
  return useContext(Ctx);
}

type Density = "comfortable" | "compact";

const DensityCtx = createContext<{ density: Density; setDensity: (d: Density) => void }>({
  density: "comfortable",
  setDensity: () => {},
});

export function DensityProvider({ children }: { children: React.ReactNode }) {
  const [density, setDensity] = useState<Density>(() =>
    localStorage.getItem("liber-density") === "compact" ? "compact" : "comfortable"
  );

  useEffect(() => {
    document.documentElement.dataset.density = density;
    localStorage.setItem("liber-density", density);
  }, [density]);

  const set = useCallback((d: Density) => setDensity(d), []);
  return <DensityCtx.Provider value={{ density, setDensity: set }}>{children}</DensityCtx.Provider>;
}

export function useDensity() {
  return useContext(DensityCtx);
}
