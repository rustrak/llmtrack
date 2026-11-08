import { cn } from '@/shared/lib/utils';

/** The product name as a mark: lowercase, mono, the brand lime on "track". */
export function LlmtrackWordmark({ className }: { className?: string }) {
  return (
    <span
      className={cn(
        'font-mono text-section font-bold tracking-tight text-fg select-none',
        className,
      )}
    >
      llm<span className="text-fg-brand">track</span>
    </span>
  );
}
