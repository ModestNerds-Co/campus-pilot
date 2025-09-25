//
//  campus-pilot
//  boot.tsx - Bootstrap Boot Route
//
//  Created by Ngonidzashe Mangudya on 26/09/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { createFileRoute } from '@tanstack/react-router';
import { BootScreen } from '../modules/configs';

export const Route = createFileRoute('/boot')({
  component: BootScreen,
});
