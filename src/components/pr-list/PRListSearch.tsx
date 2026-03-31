import { Input } from "~/components/ui/Input";

interface PRListSearchProps {
  value: string;
  onChange: (value: string) => void;
}

export function PRListSearch(props: PRListSearchProps) {
  return (
    <Input
      type="text"
      placeholder="Filter pull requests..."
      value={props.value}
      onInput={(e) => props.onChange(e.currentTarget.value)}
      class="text-xs"
    />
  );
}
