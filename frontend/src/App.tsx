import { useQuery } from "@tanstack/react-query";
import { fetchBookmarks } from "./api";

export default function App() {
  const q = useQuery({ queryKey: ["bookmarks"], queryFn: fetchBookmarks });
  return (
    <main>
      <h1>liber</h1>
      {q.isLoading && <p>loading</p>}
      {q.data && <p>{q.data.total} bookmarks</p>}
    </main>
  );
}
