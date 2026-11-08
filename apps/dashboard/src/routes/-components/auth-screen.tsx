import { Text } from '@rustrak/ui';
import { type CSSProperties, type ReactNode, useEffect, useRef } from 'react';
import { useTranslations } from 'use-intl';
import { LlmtrackWordmark } from '@/shared/ui/components/llmtrack-wordmark';
import './login.css';
import { runStrikes } from './login-strikes';

// The AI providers a key reaches, circling the gateway and striking it.
const RINGS = [
  {
    radius: '24rem',
    duration: '180s',
    providers: [
      'openai',
      'claude',
      'gemini',
      'mistral',
      'meta',
      'deepseek',
      'xai',
      'qwen',
    ],
  },
  {
    radius: '17rem',
    duration: '130s',
    providers: [
      'perplexity',
      'cohere',
      'groq',
      'ollama',
      'nvidia',
      'huggingface',
    ],
  },
  {
    radius: '10rem',
    duration: '90s',
    providers: ['openrouter', 'vllm', 'together', 'fireworks'],
  },
];

/** The providers' marks (LobeHub's icons, MIT), inlined so `currentColor` works. */
const LOGOS = import.meta.glob<string>('./ai-logos/*.svg', {
  query: '?raw',
  import: 'default',
  eager: true,
});
const logo = (provider: string) => LOGOS[`./ai-logos/${provider}.svg`] ?? '';

/**
 * The frame of the signed-out screens (login, an invitation, an assistant's
 * consent), after Rustrak's login: the sheet on the left holds the brand, the
 * providers circling it, and what llmtrack is; the right column holds the
 * screen's own form.
 */
export function AuthScreen({ children }: { children: ReactNode }) {
  const t = useTranslations('auth');
  const stats = [
    { value: '<1ms', label: t('statOverhead') },
    { value: '1', label: t('statEndpoint') },
    { value: '$', label: t('statSpend') },
  ];
  return (
    <main className="flex min-h-screen bg-canvas">
      <aside className="relative my-frame ms-frame hidden flex-[1.2] overflow-hidden rounded-3xl border border-border-subtle bg-panel lg:block">
        <LoginOrbit />
        <div
          aria-hidden="true"
          className="pointer-events-none absolute inset-x-0 bottom-0 h-72 bg-linear-to-t from-panel from-50% to-transparent"
        />
        <div
          aria-hidden="true"
          className="pointer-events-none absolute inset-0 backdrop-blur-[6px] mask-[radial-gradient(ellipse_at_50%_44%,transparent_50%,#000_95%)]"
        />
        <div
          aria-hidden="true"
          className="login-vignette pointer-events-none absolute inset-0 rounded-3xl"
        />
        <div className="login-rise absolute inset-x-0 bottom-0 flex flex-col gap-2 p-10 [animation-delay:300ms]">
          <p className="text-page-title text-fg">
            {t('heroTitle')}
            <span className="text-fg-brand">.</span>
          </p>
          <Text render={<p />} tone="muted" className="max-w-md">
            {t('heroDescription')}
          </Text>
          <dl className="mt-4 flex items-center gap-8">
            {stats.map((stat) => (
              <div key={stat.label} className="flex flex-col-reverse">
                <dt className="text-meta text-fg-subtle">{stat.label}</dt>
                <dd className="font-mono text-numeric text-fg-brand">
                  {stat.value}
                </dd>
              </div>
            ))}
          </dl>
        </div>
      </aside>

      <div className="flex min-w-0 flex-1 flex-col gap-10 p-8 lg:p-10">
        {/* On a wide screen the mark is at the centre of the panel. */}
        <LlmtrackWordmark className="login-rise w-fit lg:hidden" />

        <div className="login-stagger mx-auto my-auto w-full max-w-[24rem]">
          {children}
        </div>

        <p className="login-rise mx-auto font-mono text-meta text-fg-subtle [animation-delay:650ms]">
          &copy; {new Date().getFullYear()} llmtrack
        </p>
      </div>
    </main>
  );
}

/**
 * The AI providers a key reaches, circling the gateway and striking it with
 * light (`login-strikes.ts`), as Rustrak's SDKs strike Rustrak. Each ring
 * turns as one, and each tile turns back at the same speed so its logo stays
 * upright.
 */
function LoginOrbit() {
  const stage = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const stageEl = stage.current;
    const canvasEl = canvas.current;
    if (!stageEl || !canvasEl) return;

    // The effect reads the theme's colours when it starts, so a new theme, like
    // a new motion preference, means starting it over.
    const motion = matchMedia('(prefers-reduced-motion: reduce)');
    let stop: (() => void) | undefined;
    const restart = () => {
      stop?.();
      stop = motion.matches ? undefined : runStrikes(canvasEl, stageEl);
    };
    restart();

    motion.addEventListener('change', restart);
    const theme = new MutationObserver(restart);
    theme.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['data-theme'],
    });

    return () => {
      stop?.();
      motion.removeEventListener('change', restart);
      theme.disconnect();
    };
  }, []);

  return (
    <>
      <div
        ref={stage}
        aria-hidden="true"
        className="login-fade pointer-events-none absolute top-[44%] left-1/2 size-[64rem] -translate-1/2"
      >
        <div
          data-orbit-mark
          className="absolute top-1/2 left-1/2 -translate-1/2"
        >
          <span
            data-word
            className="block font-mono text-[1.75rem] leading-none font-bold tracking-tight"
          >
            {[...'llmtrack'].map((letter, index) => (
              <span
                // The word is fixed, so a letter is its place in it.
                key={index}
                data-letter
                className={index < 3 ? 'text-fg' : 'text-fg-brand'}
              >
                {letter}
              </span>
            ))}
          </span>
        </div>
        {RINGS.map(({ radius, duration, providers }) => (
          <div
            key={radius}
            style={
              {
                '--orbit-duration': duration,
                width: `calc(2 * ${radius})`,
                height: `calc(2 * ${radius})`,
              } as CSSProperties
            }
            className="login-orbit absolute inset-0 m-auto rounded-pill border border-border"
          >
            {providers.map((provider, index) => {
              const angle = `${(360 / providers.length) * index}deg`;
              return (
                <div
                  key={provider}
                  className="absolute top-1/2 left-1/2"
                  style={{
                    transform: `translate(-50%, -50%) rotate(${angle}) translateY(calc(-1 * ${radius})) rotate(calc(-1 * ${angle}))`,
                  }}
                >
                  <div
                    data-orbit-tile
                    className="login-orbit-tile flex size-11 items-center justify-center rounded-xl border border-border-subtle bg-surface text-fg [&>svg]:size-6"
                    // Local, static SVG files from this repository.
                    // biome-ignore lint/security/noDangerouslySetInnerHtml: see above.
                    dangerouslySetInnerHTML={{ __html: logo(provider) }}
                  />
                </div>
              );
            })}
          </div>
        ))}
      </div>
      <canvas
        ref={canvas}
        className="pointer-events-none absolute inset-0 size-full"
      />
    </>
  );
}

/** A screen's heading in the auth column: the title and one line under it. */
export function AuthHeading({
  title,
  children,
}: {
  title: ReactNode;
  children?: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-1">
      <h1 className="text-page-title text-fg">{title}</h1>
      {children && (
        <Text render={<p />} tone="muted">
          {children}
        </Text>
      )}
    </div>
  );
}
