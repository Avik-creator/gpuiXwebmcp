import { Link, Outlet, createRootRoute } from "@tanstack/react-router";
import { SiteFooter } from "../components/site-footer";
import { SiteHeader } from "../components/site-header";
import "../styles.css";

export const Route = createRootRoute({
  component: RootComponent,
  notFoundComponent: NotFound,
});

function RootComponent() {
  return (
    <div className="flex min-h-dvh flex-col bg-paper text-ink">
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
    <main id="main" className="mx-auto max-w-[640px] px-6 pt-24 sm:px-0">
      <h1 className="t-focus m-0 font-normal">Nothing here</h1>
      <p className="mt-6 text-mute">That address is not part of this site.</p>
      <p className="mt-10">
        <Link to="/" className="act">
          ‹ Back to the overview
        </Link>
      </p>
    </main>
  );
}
