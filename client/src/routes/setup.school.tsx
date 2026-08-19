//
//  campus-pilot
//  setup.school.tsx - School Setup Route
//
//  Created by Ngonidzashe Mangudya on 26/09/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { createFileRoute } from '@tanstack/react-router';
import { SchoolSetupScreen } from '../modules/configs';

export const Route = createFileRoute('/setup/school')({
  component: SchoolSetupScreen,
});
