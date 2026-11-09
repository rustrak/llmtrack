import {
  AiChat02Icon,
  ChartColumnIcon,
  ContactBookIcon,
  CubeIcon,
  DollarCircleIcon,
  Key01Icon,
  LicenseIcon,
  McpServerIcon,
  SlidersHorizontalIcon,
  Tag01Icon,
  UserMultipleIcon,
  UserSettings01Icon,
} from '@hugeicons/core-free-icons';
import { fromHugeicons } from '@rustrak/ui';

/**
 * The icons llmtrack needs that the design system's catalogue does not name:
 * the gateway's own records. Same adapter, same family, same stroke, so they
 * sit beside the catalogue's without a seam. Named for what they mean.
 */
export const KeysIcon = fromHugeicons('KeysIcon', Key01Icon);
export const ModelsIcon = fromHugeicons('ModelsIcon', CubeIcon);
export const PlaygroundIcon = fromHugeicons('PlaygroundIcon', AiChat02Icon);
export const TeamsIcon = fromHugeicons('TeamsIcon', UserMultipleIcon);
export const PeopleIcon = fromHugeicons('PeopleIcon', ContactBookIcon);
export const LabelsIcon = fromHugeicons('LabelsIcon', Tag01Icon);
export const UsageIcon = fromHugeicons('UsageIcon', ChartColumnIcon);
export const RequestLogsIcon = fromHugeicons('RequestLogsIcon', LicenseIcon);
export const UsersIcon = fromHugeicons('UsersIcon', UserSettings01Icon);
export const PricingIcon = fromHugeicons('PricingIcon', DollarCircleIcon);
export const McpIcon = fromHugeicons('McpIcon', McpServerIcon);
export const GeneralIcon = fromHugeicons('GeneralIcon', SlidersHorizontalIcon);
