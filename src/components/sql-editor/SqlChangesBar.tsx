import { Save } from "lucide-react";
import { Button } from "../ui/button";

interface SqlChangesBarProps {
  totalChanges: number;
  rowCount: number;
  tableCount: number;
  saveChanges: () => void;
  discardChanges: () => void;
  isSaving: boolean;
}

export function SqlChangesBar({
  totalChanges,
  rowCount,
  tableCount,
  saveChanges,
  discardChanges,
  isSaving,
}: SqlChangesBarProps) {
  return (
    <div className="shrink-0 px-4 py-3 border-t bg-muted/50 flex items-center justify-between">
      <span className="text-muted-foreground">
        {totalChanges} change
        {totalChanges !== 1 ? "s" : ""} to {rowCount} row
        {rowCount !== 1 ? "s" : ""}
        {tableCount > 1 && ` across ${tableCount} tables`}
      </span>
      <div className="flex items-center gap-2">
        <Button
          variant="outline"
          onClick={discardChanges}
          disabled={isSaving}
          size="sm"
        >
          Cancel
        </Button>
        <Button onClick={saveChanges} disabled={isSaving} size="sm">
          <Save size={14} />
          {isSaving ? "Saving..." : "Save"}
        </Button>
      </div>
    </div>
  );
}
