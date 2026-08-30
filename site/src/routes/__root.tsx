import { Outlet, createRootRoute } from "@tanstack/react-router";
import { SiteFooter } from "../components/site-footer";
import { SiteHeader } from "../components/site-header";
import "../styles.css";

export const Route = createRootRoute({
  component: RootComponent,
  notFoundComponent: NotFound,
});

function RootComponent() {
  return (
    <div className="flex min-h-dvh flex-col bg-background text-foreground">
      <SiteHeader />
      <div className="flex-1">
        <Outlet />
      </div>
      <SiteFooter />
    </div>
  );
}

function NotFound() {
  return (
    <main id="main" className="mx-auto max-w-3xl px-4 py-16 sm:px-6">
      <h1 className="text-2xl font-semibold">Page not found</h1>
      <p className="mt-3 text-muted-foreground">
        That route is not part of this explainer. Go back to the home page.
      </p>
    </main>
  );
}
