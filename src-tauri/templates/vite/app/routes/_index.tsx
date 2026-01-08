import type { Route } from "./+types/_index";
import { Link } from "react-router";
import { Button } from "~/components/ui/button";

export function meta({}: Route.MetaArgs) {
  return [
    { title: "Home" },
    { name: "description", content: "Welcome to the app!" },
  ];
}

export default function Home() {
  return (
    <div className="flex h-screen flex-col items-center justify-center gap-6">
      <h1 className="text-4xl font-bold">Welcome</h1>
      <div className="flex gap-4">
        <Button asChild>
          <Link to="/login">Login</Link>
        </Button>
        <Button asChild variant="outline">
          <Link to="/sign-up">Sign Up</Link>
        </Button>
      </div>
    </div>
  );
}
