# Configs Module - CampusPilot Bootstrap System

## Overview

The configs module handles the initial setup and configuration of CampusPilot. It provides a complete first-run experience that guides users through school configuration and administrator account creation.

## Architecture

### Module Structure

```
src/modules/configs/
├── components/           # Module-specific UI components
│   ├── screens/         # Full-page screens
│   │   ├── BootScreen.tsx          # Bootstrap status checking
│   │   ├── SchoolSetupScreen.tsx   # School configuration form
│   │   └── AdminSetupScreen.tsx    # Admin account creation
│   └── ui/              # Reusable UI components
│       └── SchoolPreviewCard.tsx   # Live preview component
├── services/            # Module-specific services
│   └── bootstrapService.ts         # API communication service
├── types/               # TypeScript definitions
│   └── index.ts         # All module types
├── constants/           # Configuration constants
│   └── index.ts         # API endpoints, validation rules, etc.
└── index.ts            # Module exports
```

### App-wide Integration

```
src/
├── lib/                 # App-wide utilities
│   └── validation.ts    # Shared validation functions
├── routes/              # App-wide routing
│   ├── boot.tsx         # Bootstrap entry point
│   ├── setup.school.tsx # School setup route
│   ├── setup.admin.tsx  # Admin setup route
│   └── login.tsx        # Login screen route
└── components/          # App-wide components
    └── LoginScreen.tsx  # Login interface
```

## Bootstrap Flow

### 1. Initial Load (`/`)
- App checks bootstrap status via `bootstrapService.checkStatus()`
- Routes to appropriate screen based on state:
  - `Uninitialized` → `/setup/school`
  - `SchoolConfigured` → `/setup/admin`  
  - `Ready` → `/login`

### 2. Boot Screen (`/boot`)
- Shows loading spinner while checking configuration
- Handles offline scenarios gracefully
- Provides retry functionality for failed status checks

### 3. School Setup (`/setup/school`)
- **Form Fields:**
  - School name (required)
  - Legal name, EMAP code
  - Contact info (email, phone)
  - Address details
  - Timezone and locale selection
  - Logo uploads (light/dark variants)

- **Features:**
  - Real-time form validation
  - Live preview showing login screen and receipt header
  - Image upload with validation (size, format, dimensions)
  - Offline-friendly operation
  - Base64 encoding for logo storage

### 4. Admin Setup (`/setup/admin`)
- **Form Fields:**
  - Full name (required)
  - Email address (required, becomes login)
  - Phone number (optional)
  - Password with strength meter
  - Password confirmation

- **Features:**
  - Password strength validation (10+ chars, number, symbol)
  - Caps Lock detection
  - Real-time password matching
  - Security recommendations

### 5. Login Screen (`/login`)
- Uses school branding from configuration
- Shows school logo and contact information
- Standard email/password authentication
- Responsive design with accessibility features

## API Integration

### Mock vs Production Mode

The system includes a mock mode for development:

```typescript
// In bootstrapService.ts
private isMockMode = true; // Set to false when real API is ready
```

### API Endpoints

- `GET /api/1.0/bootstrap/status` - Check bootstrap state
- `POST /api/1.0/bootstrap/school` - Configure school settings
- `POST /api/1.0/bootstrap/admin` - Create administrator account

### Response Format

All APIs use a consistent envelope:

```typescript
{
  "success": boolean,
  "message": string | null,
  "data": any | null,
  "issues": [{ code: string, detail: string, field?: string }] | null,
  "version": "1.0",
  "by": "CampusPilot"
}
```

## Key Features

### Offline Support
- Forms work without internet connectivity
- Changes cached locally until connection restored
- Visual indicators for offline status

### Accessibility
- All forms have proper labels and ARIA attributes
- Keyboard navigation support
- High contrast ratios (4.5:1 minimum)
- Screen reader friendly

### Validation
- Client-side validation with real-time feedback
- Server-side validation with field-specific errors
- Progressive enhancement approach

### Security
- Password strength requirements
- Base64 encoding for sensitive data
- No credential exposure in logs or errors

### User Experience
- Friendly, confident tone in copy
- Loading states and progress indicators
- Error recovery with retry mechanisms
- Live previews showing final result

## State Management

### Bootstrap State Flow

```
Uninitialized → SchoolConfigured → Ready
     ↓                ↓              ↓
/setup/school   /setup/admin    /login
```

### Local Storage
- Mock bootstrap state stored in `campuspilot_bootstrap_state`
- Includes school data, admin data, and current state
- Automatically cleared when real API is implemented

## Customization

### Branding
- Primary color derived from uploaded logo (future enhancement)
- School name and logo used throughout interface
- Timezone and locale settings applied globally

### Validation Rules
Configurable in `constants/index.ts`:
- Password requirements
- Image size limits
- Field length constraints
- Required field definitions

## Testing

### Development Testing
1. Start dev server: `npm run dev`
2. Navigate to `http://localhost:3001`
3. App automatically starts bootstrap flow
4. Complete school setup → admin creation → login

### Reset Bootstrap State
```typescript
// In browser console
bootstrapService.resetMockState();
window.location.reload();
```

## Future Enhancements

- [ ] Auto-theme color extraction from logos
- [ ] Multi-language support based on locale
- [ ] Import wizard for existing data
- [ ] Advanced password policies
- [ ] Two-factor authentication setup
- [ ] Email verification for admin accounts

## Dependencies

- **React 18+** - UI framework
- **TanStack Router** - Routing and navigation
- **TanStack Query** - Data fetching (if needed)
- **Tailwind CSS** - Styling
- **Lucide React** - Icons
- **React Hot Toast** - Notifications
- **Zod** - Runtime validation (future)

## Migration from Mock to Production

1. Set `isMockMode = false` in `bootstrapService.ts`
2. Implement actual API endpoints matching the interface
3. Remove mock storage cleanup code
4. Add proper error handling for network failures
5. Implement authentication tokens and session management