import type { FormConfig, FormField } from "@/specs/types";
import { useCallback } from "react";
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
  formConfig: FormConfig;
  formValues: Record<string, unknown>;
  onFormValuesChange: (values: Record<string, unknown>) => void;
  onSubmit: () => void;
}

/**
 * Renders a form for mutation items - error display is handled by parent QueryBlock
 * Data loading for edit forms is handled by parent QueryBlock
 */
export function SqlFormArea({
  formConfig,
  formValues,
  onFormValuesChange,
  onSubmit,
}: SqlFormAreaProps) {
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

  return (
    <form
      onSubmit={handleSubmit}
      className="flex-1 flex flex-col overflow-hidden pb-12"
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
    </form>
  );
}
