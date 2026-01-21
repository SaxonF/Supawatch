import { useForm } from "@tanstack/react-form";
import { zodValidator } from "@tanstack/zod-form-adapter";
import { useEffect, useState } from "react";
import { z } from "zod";
import * as api from "../api";
import { Button } from "./ui/button";
import { Field, FieldDescription, FieldGroup, FieldLabel } from "./ui/field";
import { Input } from "./ui/input";

export function Settings() {
  return (
    <div className="space-y-8 max-w-lg">
      <div className="space-y-4">
        <p className="text-muted-foreground text-sm">
          Supawatch monitors your local Supabase project folders for changes to
          schema files and edge functions, then syncs them to your remote
          Supabase project.
        </p>
      </div>

      <AccessTokenForm />
      <OpenAIKeyForm />
    </div>
  );
}

function AccessTokenForm() {
  const [hasToken, setHasToken] = useState(false);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    checkToken();
  }, []);

  const checkToken = async () => {
    try {
      const has = await api.hasAccessToken();
      setHasToken(has);
    } finally {
      setIsLoading(false);
    }
  };

  const form = useForm({
    defaultValues: {
      token: "",
    },
    // @ts-ignore
    validatorAdapter: zodValidator(),
    validators: {
      onSubmit: z.object({
        token: z.string().min(1, "Access token is required"),
      }),
    },
    onSubmit: async ({ value }) => {
      try {
        await api.setAccessToken(value.token.trim());
        const isValid = await api.validateAccessToken();

        if (isValid) {
          setHasToken(true);
          form.reset();
        } else {
          await api.clearAccessToken();
          form.setErrorMap({
            // @ts-ignore
            onSubmit: "Invalid access token. Please check and try again.",
          });
        }
      } catch (err) {
        form.setErrorMap({
          // @ts-ignore
          // @ts-ignore
          onSubmit: String(err),
        });
      }
    },
  });

  const handleClear = async () => {
    try {
      await api.clearAccessToken();
      setHasToken(false);
    } catch (err) {
      console.error(err);
    }
  };

  if (isLoading) return null;

  return (
    <div className="space-y-4">
      <FieldGroup>
        <Field>
          <FieldLabel>Supabase Personal Access Token</FieldLabel>
          {hasToken ? (
            <div className="flex gap-2">
              <Input
                readOnly
                value="sbp_xxxxxxxxxxxxxxxxxxxxxxxx"
                className="flex-1 bg-muted text-muted-foreground"
              />
              <Button variant="outline" onClick={handleClear}>
                Clear
              </Button>
            </div>
          ) : (
            <form
              onSubmit={(e) => {
                e.preventDefault();
                e.stopPropagation();
                form.handleSubmit();
              }}
            >
              <form.Field
                name="token"
                children={(field) => (
                  <div className="flex gap-2">
                    <div className="flex-1">
                      <Input
                        type="password"
                        placeholder="sbp_xxxxxxxxxxxxxxxxxxxxxxxx"
                        value={field.state.value}
                        onChange={(e) => field.handleChange(e.target.value)}
                        className={
                          field.state.meta.errors.length > 0
                            ? "border-destructive"
                            : ""
                        }
                      />
                    </div>
                    <form.Subscribe
                      selector={(state) => [
                        state.canSubmit,
                        state.isSubmitting,
                      ]}
                      children={([canSubmit, isSubmitting]) => (
                        <Button
                          type="submit"
                          disabled={!canSubmit || isSubmitting}
                        >
                          {isSubmitting ? "Saving..." : "Save"}
                        </Button>
                      )}
                    />
                  </div>
                )}
              />
              <form.Subscribe
                selector={(state) => [state.errors]}
                children={([errors]) =>
                  errors.length > 0 ? (
                    <div className="mt-2 text-sm text-destructive">
                      {errors.map((e) => e?.toString()).join(", ")}
                    </div>
                  ) : null
                }
              />
            </form>
          )}
          <FieldDescription>
            You can generate a new token in your Supabase Account Settings.
          </FieldDescription>
        </Field>
      </FieldGroup>
    </div>
  );
}

function OpenAIKeyForm() {
  const [hasKey, setHasKey] = useState(false);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    checkKey();
  }, []);

  const checkKey = async () => {
    try {
      const has = await api.hasOpenAiKey();
      setHasKey(has);
    } finally {
      setIsLoading(false);
    }
  };

  const form = useForm({
    defaultValues: {
      key: "",
    },
    // @ts-ignore
    validatorAdapter: zodValidator(),
    validators: {
      onSubmit: z.object({
        key: z.string().min(1, "API key is required"),
      }),
    },
    onSubmit: async ({ value }) => {
      try {
        await api.setOpenAiKey(value.key.trim());
        setHasKey(true);
        form.reset();
      } catch (err) {
        form.setErrorMap({
          // @ts-ignore
          // @ts-ignore
          onSubmit: String(err),
        });
      }
    },
  });

  const handleClear = async () => {
    try {
      await api.clearOpenAiKey();
      setHasKey(false);
    } catch (err) {
      console.error(err);
    }
  };

  if (isLoading) return null;

  return (
    <div className="space-y-4">
      <FieldGroup>
        <Field>
          <FieldLabel>OpenAI API Key</FieldLabel>
          {hasKey ? (
            <div className="flex gap-2">
              <Input
                readOnly
                value="sk-xxxxxxxxxxxxxxxxxxxxxxxx"
                className="flex-1 bg-muted text-muted-foreground"
              />
              <Button variant="outline" onClick={handleClear}>
                Clear
              </Button>
            </div>
          ) : (
            <form
              onSubmit={(e) => {
                e.preventDefault();
                e.stopPropagation();
                form.handleSubmit();
              }}
            >
              <form.Field
                name="key"
                children={(field) => (
                  <div className="flex gap-2">
                    <div className="flex-1">
                      <Input
                        type="password"
                        placeholder="sk-xxxxxxxxxxxxxxxxxxxxxxxx"
                        value={field.state.value}
                        onChange={(e) => field.handleChange(e.target.value)}
                        className={
                          field.state.meta.errors.length > 0
                            ? "border-destructive"
                            : ""
                        }
                      />
                    </div>
                    <form.Subscribe
                      selector={(state) => [
                        state.canSubmit,
                        state.isSubmitting,
                      ]}
                      children={([canSubmit, isSubmitting]) => (
                        <Button
                          type="submit"
                          disabled={!canSubmit || isSubmitting}
                        >
                          {isSubmitting ? "Saving..." : "Save"}
                        </Button>
                      )}
                    />
                  </div>
                )}
              />
              <form.Subscribe
                selector={(state) => [state.errors]}
                children={([errors]) =>
                  errors.length > 0 ? (
                    <div className="mt-2 text-sm text-destructive">
                      {errors.map((e) => e?.toString()).join(", ")}
                    </div>
                  ) : null
                }
              />
            </form>
          )}
          <FieldDescription>
            Used for natural language to SQL conversion in the SQL editor.
          </FieldDescription>
        </Field>
      </FieldGroup>
    </div>
  );
}
