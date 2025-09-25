//
//  campus-pilot
//  login.tsx - Login Route
//
//  Created by Ngonidzashe Mangudya on 26/09/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { createFileRoute } from '@tanstack/react-router';
import { LoginScreen } from '../components/LoginScreen';

export const Route = createFileRoute('/login')({
  component: LoginScreen,
});
