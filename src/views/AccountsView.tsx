import { LogIn, UserCircle2 } from "lucide-react";

import { Button, EmptyState, PageHeader } from "../components/ui";

export function AccountsView() {
  return (
    <div className="flex flex-1 flex-col">
      <PageHeader title="Accounts" subtitle="Sign in with your Microsoft account to play online." />
      <EmptyState
        icon={<UserCircle2 className="size-6" />}
        title="No accounts"
        description="Microsoft sign-in arrives in the authentication milestone. This is where your profiles will live."
        action={
          <Button variant="ghost">
            <LogIn className="size-4" />
            Add Microsoft account
          </Button>
        }
      />
    </div>
  );
}
