import { useState } from "react";

export function Login() {
  const [error, setError] = useState("");
  return (
    <div className="flex min-h-screen items-center justify-center bg-neutral-50 dark:bg-neutral-950">
      <form
        className="w-full max-w-sm rounded-2xl border border-neutral-200 bg-white p-6 shadow-sm dark:border-neutral-800 dark:bg-neutral-900"
        method="post"
        action="/login"
        onSubmit={() => setError("")}
      >
        <h1 className="text-lg font-semibold">liber</h1>
        <p className="mb-4 text-sm text-neutral-500">This server requires the auth token.</p>
        {error && <p className="mb-2 text-sm text-red-600">{error}</p>}
        <input
          type="password"
          name="token"
          placeholder="Auth token"
          required
          autoFocus
          className="w-full rounded-lg border border-neutral-300 px-3 py-1.5 text-sm outline-none focus:border-accent-500 dark:border-neutral-700 dark:bg-neutral-800"
        />
        <input type="hidden" name="next" value="/" />
        <button type="submit" className="mt-3 w-full rounded-lg bg-accent-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-accent-700">
          Log in
        </button>
      </form>
    </div>
  );
}
