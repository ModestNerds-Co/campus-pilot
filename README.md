# CampusPilot

> **Modern School Management System**
> A comprehensive platform for educational institutions built with React, TypeScript, and modern web technologies.

## 🏗️ Architecture

CampusPilot follows a **modular architecture** with clear separation of concerns:

```
src/
├── modules/             # Feature modules (self-contained)
├── components/          # App-wide shared components
├── lib/                 # App-wide utilities and services
├── routes/              # App-wide routing definitions
└── hooks/               # App-wide custom hooks
```

## 📚 Modules

### 🔧 [Configs Module](./src/modules/configs/README.md)
**Bootstrap & First-Run Setup System**
- School configuration and branding setup
- Administrator account creation
- Complete onboarding experience
- Offline-friendly operation

*Status: ✅ Complete - Production Ready*

### 🎓 Student Management *(Coming Soon)*
**Student Information System**
- Student enrollment and profiles
- Academic records and transcripts
- Parent/guardian management

### 💰 Financial Management *(Coming Soon)*
**Fees and Payments System**
- Fee structure management
- Payment processing and tracking
- Financial reporting

### 📊 Academic Management *(Coming Soon)*
**Curriculum and Assessment**
- Course and class management
- Grading and assessment tools
- Academic calendar

### 👥 Staff Management *(Coming Soon)*
**Human Resources System**
- Staff profiles and roles
- Attendance tracking
- Performance management

## 🚀 Quick Start

### Prerequisites
- **Node.js 18+**
- **npm** or **pnpm**

### Development Setup

```bash
# Clone the repository
git clone <repository-url>
cd campuspilot/app

# Install dependencies
npm install

# Start development server
npm run dev

# Open browser to http://localhost:3000
```

### First Run Experience

1. **Navigate to the app** - Opens bootstrap flow automatically
2. **Configure School** - Set up branding, contact info, and preferences
3. **Create Admin** - Set up the first administrator account
4. **Access System** - Log in and start using CampusPilot

## 🛠️ Development

### Scripts

```bash
npm run dev         # Start development server
npm run build       # Build for production
npm run preview     # Preview production build
npm run typecheck   # Run TypeScript checks
```

### Code Organization

#### Modules vs App-wide Code

**Modules** (`src/modules/*/`)
- Feature-specific components
- Module-specific services
- Module-specific types
- Module constants

**App-wide** (`src/lib/`, `src/components/`, etc.)
- Shared utilities and validation
- Reusable UI components
- Global hooks and services
- Routing definitions

#### Adding New Modules

1. Create module directory: `src/modules/your-module/`
2. Follow the standard structure:
   ```
   src/modules/your-module/
   ├── components/
   ├── services/
   ├── types/
   ├── constants/
   ├── README.md
   └── index.ts
   ```
3. Export from module index
4. Document in main README

### Technology Stack

#### Core Framework
- **React 18** - UI framework with concurrent features
- **TypeScript** - Type safety and developer experience
- **Vite** - Fast build tool and dev server

#### Routing & State
- **TanStack Router** - Type-safe routing with file-based routing
- **TanStack Query** - Data fetching and caching
- **Zustand** - Lightweight state management (where needed)

#### Styling & UI
- **Tailwind CSS** - Utility-first CSS framework
- **Lucide React** - Beautiful icon library
- **React Hot Toast** - Toast notifications

#### Development Tools
- **ESLint** - Code linting
- **Prettier** - Code formatting
- **TypeScript** - Static type checking

## 📖 API Documentation

### Response Format

All APIs use a consistent envelope format:

```typescript
interface ApiEnvelope<T> {
  success: boolean;
  message: string | null;
  data: T | null;
  issues: Array<{
    code: string;
    detail: string;
    field?: string;
  }> | null;
  version: string;
  by: string;
}
```

### Error Handling

- **Client-side validation** with real-time feedback
- **Server-side validation** with field-specific errors
- **Network error recovery** with retry mechanisms
- **Offline operation** where possible

## 🔐 Security

### Authentication & Authorization
- Secure password requirements (10+ chars, numbers, symbols)
- Role-based access control
- Session management
- CSRF protection

### Data Protection
- Input validation and sanitization
- SQL injection prevention
- XSS protection
- Secure file upload handling

## 🌍 Internationalization

### Supported Locales
- **English (Zimbabwe)** - `en-ZW` *(Primary)*
- **Shona (Zimbabwe)** - `sn-ZW`
- **Ndebele (Zimbabwe)** - `nd-ZW`
- **English (US)** - `en-US`
- **English (UK)** - `en-GB`

### Timezone Support
- **Africa/Harare** *(Default)*
- **Africa/Johannesburg**
- **UTC** and other major timezones

## 🧪 Testing

### Testing Strategy
- **Unit Tests** - Component and utility testing
- **Integration Tests** - API and workflow testing
- **E2E Tests** - Complete user journey testing
- **Accessibility Tests** - WCAG compliance verification

### Running Tests
```bash
npm run test          # Run all tests
npm run test:unit     # Unit tests only
npm run test:e2e      # End-to-end tests
npm run test:a11y     # Accessibility tests
```

## 🚢 Deployment

### Production Build
```bash
npm run build
npm run preview  # Test production build locally
```

### Environment Configuration
```bash
# .env.production
VITE_API_BASE_URL=https://api.yourschool.com
VITE_APP_VERSION=1.0.0
```

### Docker Deployment
```dockerfile
# See Dockerfile for containerized deployment
docker build -t campuspilot .
docker run -p 3000:3000 campuspilot
```

## 📈 Performance

### Optimization Features
- **Code splitting** - Route-based lazy loading
- **Image optimization** - Automatic compression and formats
- **Bundle analysis** - Size monitoring and optimization
- **Caching strategies** - Aggressive caching for static assets

### Performance Targets
- **First Contentful Paint** < 2s
- **Largest Contentful Paint** < 3s
- **Time to Interactive** < 4s
- **Cumulative Layout Shift** < 0.1

## ♿ Accessibility

### WCAG 2.1 AA Compliance
- **Semantic HTML** - Proper markup structure
- **Keyboard Navigation** - Full keyboard accessibility
- **Screen Reader Support** - ARIA labels and descriptions
- **Color Contrast** - Minimum 4.5:1 ratio
- **Focus Management** - Clear focus indicators

## 🤝 Contributing

### Development Workflow
1. **Fork** the repository
2. **Create** feature branch: `git checkout -b feature/amazing-feature`
3. **Commit** changes: `git commit -m 'Add amazing feature'`
4. **Push** to branch: `git push origin feature/amazing-feature`
5. **Open** Pull Request

### Code Standards
- **TypeScript** - Strict mode enabled
- **ESLint** - Airbnb configuration
- **Prettier** - Consistent formatting
- **Conventional Commits** - Clear commit messages

### Documentation
- Update README.md for new features
- Add module documentation
- Include JSDoc comments for functions
- Update API documentation

## 📄 License

Copyright (c) 2025 Codecraft Solutions. All rights reserved.

## 🆘 Support

### Getting Help
- **Issues** - Report bugs and feature requests
- **Discussions** - Community questions and ideas
- **Documentation** - Comprehensive guides and tutorials
- **Support Email** - Technical support contact

### Quick Links
- [🔧 Configs Module Documentation](./src/modules/configs/README.md)
- [🔗 API Reference](#api-documentation)
- [🎨 UI Components Guide](#)
- [🧪 Testing Guide](#testing)
- [🚀 Deployment Guide](#deployment)

---

**Built with ❤️ for educational institutions worldwide**
