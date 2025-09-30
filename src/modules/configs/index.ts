//
//  campus-pilot
//  index.ts - Configs Module Exports
//
//  Created by Ngonidzashe Mangudya on 26/09/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

// Types
export type * from "./types";

// Constants
export * from "./constants";

// Services
export { bootstrapService } from "./services/bootstrap-service";

// Components - Screens
export { BootScreen } from "./components/screens/boot-screen";
export { SchoolSetupScreen } from "./components/screens/school-setup-screen";
export { AdminSetupScreen } from "./components/screens/admin-setup-screen";

// Components - UI
export { SchoolPreviewCard } from "./components/ui/school-preview-card";
