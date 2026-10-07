import { cn } from '@/shared/lib/utils';

/** The product name as a mark: lowercase, mono, the brand lime on "track". */
export function LlmtrackWordmark({ className }: { className?: string }) {
  return (
    <span
      className={cn(
        'font-mono text-lg font-bold tracking-tight select-none',
        className,
      )}
    >
      llm<span className="text-primary">track</span>
    </span>
  );
}
