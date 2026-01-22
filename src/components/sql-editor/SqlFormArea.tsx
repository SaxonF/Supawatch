import * as api from "@/api";
import type { FormConfig, FormField } from "@/specs/types";
import { Loader2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Button } from "../ui/button";
import { Field, FieldGroup, FieldLabel } from "../ui/field";
import { Input } from "../ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../ui/select";
import { Switch } from "../ui/switch";
import { Textarea } from "../ui/textarea";

interface SqlFormAreaProps {
  projectId: string;
  sql: string;
  formConfig: FormConfig;
  loadQuery?: string;
  params: Record<string, string>;
  formValues: Record<string, unknown>;
  onFormValuesChange: (values: Record<string, unknown>) => void;
  onSubmit: () => void;
  onCancel?: () => void;
  isSubmitting?: boolean;
  isProcessingWithAI?: boolean;
}

/**
 * Renders a form for mutation items - error display is handled by parent QueryBlock
 */
export function SqlFormArea({
  projectId,
  formConfig,
  loadQuery,
  params,
  formValues,
  onFormValuesChange,
  onSubmit,
  onCancel,
  isSubmitting = false,
  isProcessingWithAI = false,
}: SqlFormAreaProps) {
  const [isLoading, setIsLoading] = useState(false);

  // Load existing data if loadQuery is defined
  useEffect(() => {
    if (!loadQuery) return;

    async function loadData() {
      setIsLoading(true);

      try {
        // Interpolate params into loadQuery
        const sql = loadQuery!.replace(/:(w+)/g, (_, key) => {
          const value = params[key];
          if (value === null || value === undefined) return "NULL";
          return `'${String(value).replace(/'/g, "''")}'`;
        });

        const result = await api.runQuery(projectId, sql, true);

        if (Array.isArray(result) && result.length > 0) {
          const row = result[0] as Record<string, unknown>;
          const loaded: Record<string, unknown> = {};
          for (const field of formConfig.fields) {
            if (row[field.name] !== undefined) {
              loaded[field.name] = row[field.name];
            }
          }
          onFormValuesChange({ ...formValues, ...loaded });
        }
      } catch (err) {
        console.error("Failed to load form data:", err);
      } finally {
        setIsLoading(false);
      }
    }

    loadData();
    // Only run on mount or when loadQuery/params change
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loadQuery, projectId, JSON.stringify(params)]);

  const updateField = useCallback(
    (name: string, value: unknown) => {
      onFormValuesChange({ ...formValues, [name]: value });
    },
    [formValues, onFormValuesChange],
  );

  const renderField = (field: FormField) => {
    const value = formValues[field.name];

    switch (field.type) {
      case "text":
      case "number":
        return (
          <Input
            type={field.type}
            value={String(value ?? "")}
            onChange={(e) =>
              updateField(
                field.name,
                field.type === "number"
                  ? Number(e.target.value)
                  : e.target.value,
              )
            }
            placeholder={field.placeholder}
            required={field.required}
          />
        );

      case "textarea":
        return (
          <Textarea
            value={String(value ?? "")}
            onChange={(e) => updateField(field.name, e.target.value)}
            placeholder={field.placeholder}
            required={field.required}
          />
        );

      case "boolean":
        return (
          <Switch
            checked={Boolean(value)}
            onCheckedChange={(checked) => updateField(field.name, checked)}
          />
        );

      case "select":
        return (
          <Select
            value={String(value ?? "")}
            onValueChange={(v) => updateField(field.name, v)}
          >
            <SelectTrigger>
              <SelectValue placeholder={field.placeholder || "Select..."} />
            </SelectTrigger>
            <SelectContent>
              {field.options?.map((opt) => (
                <SelectItem key={opt.value} value={opt.value}>
                  {opt.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        );

      default:
        return (
          <Input
            value={String(value ?? "")}
            onChange={(e) => updateField(field.name, e.target.value)}
            placeholder={field.placeholder}
          />
        );
    }
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    onSubmit();
  };

  if (isLoading) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <Loader2 className="size-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  return (
    <form
      onSubmit={handleSubmit}
      className="flex-1 flex flex-col overflow-hidden"
    >
      <div className="flex-1 overflow-y-auto p-4">
        <FieldGroup className="gap-4">
          {formConfig.fields.map((field) => (
            <Field
              key={field.name}
              orientation={field.type === "boolean" ? "horizontal" : "vertical"}
            >
              <FieldLabel>{field.label}</FieldLabel>
              {renderField(field)}
            </Field>
          ))}
        </FieldGroup>
      </div>

      <div className="shrink-0 px-4 py-3 border-t bg-muted/50 flex items-center justify-end gap-2">
        {onCancel && (
          <Button type="button" variant="outline" onClick={onCancel}>
            Cancel
          </Button>
        )}
        <Button type="submit" disabled={isSubmitting || isProcessingWithAI}>
          {isSubmitting && <Loader2 className="mr-2 size-4 animate-spin" />}
          Run
        </Button>
      </div>
    </form>
  );
}
