import { Eye, EyeOff } from 'lucide-react';
import { useState } from 'react';
import { useTranslations } from 'use-intl';
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from '@/shared/ui/components/shadcn/input-group';

/** A password input with a button that shows what was typed. */
export function PasswordInput({
  className,
  'data-slot': _formSlot,
  ...props
}: Omit<React.ComponentProps<'input'>, 'type'> & { 'data-slot'?: string }) {
  const t = useTranslations('common');
  const [visible, setVisible] = useState(false);

  // `FormControl` clones its props onto this element. Its `data-slot` is
  // dropped: the input's own slot is what lights the group's focus ring.
  return (
    <InputGroup className={className} data-disabled={props.disabled}>
      <InputGroupInput type={visible ? 'text' : 'password'} {...props} />
      <InputGroupAddon align="inline-end">
        <InputGroupButton
          size="icon-xs"
          tabIndex={-1}
          disabled={props.disabled}
          onClick={() => setVisible((prev) => !prev)}
          aria-label={visible ? t('hidePassword') : t('showPassword')}
        >
          {visible ? <EyeOff /> : <Eye />}
        </InputGroupButton>
      </InputGroupAddon>
    </InputGroup>
  );
}
