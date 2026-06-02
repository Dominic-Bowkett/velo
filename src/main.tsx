import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { RouterProvider } from "@tanstack/react-router";
import { router } from "./router";
import ThreadWindow from "./ThreadWindow";
import ComposerWindow from "./ComposerWindow";
import { AuthGate } from "./components/auth/AuthGate";
import "./styles/globals.css";

const params = new URLSearchParams(window.location.search);
const isThreadWindow = params.has("thread") && params.has("account");
const isComposerWindow = params.has("compose");

function Root() {
  if (isThreadWindow) return <ThreadWindow />;
  if (isComposerWindow) return <ComposerWindow />;
  return (
    <AuthGate>
      <RouterProvider router={router} />
    </AuthGate>
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Root />
  </StrictMode>,
);
