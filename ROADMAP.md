# Frontend Roadmap

## Setup & Infrastructure
- [x] Initial project setup with Vite + React + TypeScript
- [x] TanStack Router configuration
- [x] TanStack Query setup for data fetching
- [x] Tailwind CSS + theme system (dark/light mode)
- [x] Global HTTP client with Axios (auth interceptors)
- [x] Project running successfully
- [x] Docker configuration
- [x] Command palette component
- [x] Keyboard shortcuts system

## Bootstrap & Configuration Module
- [x] Boot screen component
- [x] School setup screen (UI complete)
- [x] Admin setup screen (UI complete)
- [x] Bootstrap service with mock mode
- [x] System state management (Uninitialized → SchoolConfigured → Ready)
- [ ] Connect bootstrap flows to real backend API
- [ ] Remove mock mode once backend is ready
- [ ] Handle bootstrap error states

---

## Authentication System

### Core Authentication
- [x] Login screen component (UI complete)
- [x] Basic auth token storage in localStorage
- [x] HTTP client auth interceptor (Bearer token)
- [x] 401 redirect to login
- [ ] **Connect login to backend auth API**
  - POST /api/1.0/auth/login endpoint
  - Request: `{ email/username, password }`
  - Response: `{ access_token, refresh_token, user, expires_in }`
  - Handle validation errors
  - Handle invalid credentials
- [ ] **Remove mock login logic**
- [ ] **Implement actual authentication flow**
  - Store tokens securely
  - Store user data
  - Redirect to dashboard on success

### Authentication State Management
- [ ] **Create auth store (Zustand)**
  - User state (id, email, full_name, roles, is_active)
  - Auth status (loading, authenticated, unauthenticated)
  - Login/logout actions
  - Token management
  - User profile update
- [ ] **Persist auth state**
  - Save to localStorage
  - Rehydrate on app load
  - Handle token expiration

### Protected Routes & Guards
- [ ] **Implement route guards**
  - Check authentication before rendering protected routes
  - Redirect unauthenticated users to login
  - Handle loading states during auth check
- [ ] **Role-based route protection**
  - Check user roles for admin routes
  - Show 403 forbidden for insufficient permissions
  - Redirect based on user role

### Session Management
- [ ] **Token refresh mechanism**
  - Implement refresh token flow
  - Auto-refresh before expiration
  - Handle refresh failures
- [ ] **Session timeout handling**
  - Detect inactivity
  - Show timeout warning modal
  - Auto-logout on timeout
- [ ] **Logout functionality**
  - Clear auth state
  - Clear tokens from storage
  - Redirect to login
  - Optional: Invalidate token on server

### Additional Auth Features
- [ ] **Password reset flow**
  - Forgot password page
  - Request reset endpoint integration
  - Reset confirmation page
  - Password strength validation
- [ ] **Remember me functionality**
  - Extended session option
  - Persist login preference
- [ ] **Account security**
  - Force password change on first login
  - Password expiration warnings
  - Multi-device session management

---

## Admin Panel - User Management

### Admin Dashboard Layout
- [ ] **Create main admin layout**
  - Top navigation bar with user menu
  - Sidebar navigation with modules
  - Breadcrumbs
  - Page container with consistent spacing
- [ ] **Build admin navigation/sidebar**
  - User Management section
  - School Configuration section
  - Reports section (future)
  - Settings section
  - Role-based menu visibility
- [ ] **Admin dashboard home**
  - Statistics cards (total users, active users, roles, departments)
  - Recent activity feed
  - Quick actions
  - System status indicators

### User Management Module
- [ ] **Users list page**
  - Data table with TanStack Table
  - Columns: Name, Email, Role(s), Department, Status, Actions
  - Search/filter by name, email, role, department, status
  - Pagination
  - Bulk actions (activate, deactivate, delete)
  - Export to CSV/Excel
- [ ] **Create user modal/page**
  - Form fields: email, full_name, password, phone, roles[], department_id, section_id
  - Form validation with Zod
  - Password strength indicator
  - Role selection (multi-select)
  - Department & section dropdowns
  - Submit to POST /api/1.0/users
- [ ] **Edit user modal/page**
  - Pre-fill form with existing data
  - Allow role updates
  - Allow department/section reassignment
  - Optional password change
  - Submit to PUT /api/1.0/users/:id
- [ ] **View user details page**
  - User profile information
  - Assigned roles
  - Department & section
  - Activity log
  - Session history
- [ ] **User actions**
  - Activate/deactivate user
  - Delete user (soft delete)
  - Reset password (send reset email)
  - Force password change
  - Manage user roles

### User Roles Management
- [ ] **Roles list page**
  - Data table showing all roles
  - Columns: Role Name, Description, Users Count, Permissions, Actions
  - Search/filter
- [ ] **Create role modal/page**
  - Form: name, description
  - Module-based permissions selector
    - List all modules (User Management, Departments, etc.)
    - Permission modes: None, View, Create, Edit, Delete, Full
  - Route-level permissions (optional granular control)
  - Submit to POST /api/1.0/roles
- [ ] **Edit role modal/page**
  - Update role name, description
  - Modify permissions
  - Submit to PUT /api/1.0/roles/:id
- [ ] **Role permissions management**
  - Visual permission matrix
  - Module-level toggles
  - Route-level checkboxes per module
  - Save permissions to access_rights table
- [ ] **Delete role**
  - Confirm deletion
  - Handle users with this role (reassign or remove)
  - Submit to DELETE /api/1.0/roles/:id

---

## Admin Panel - School Structure

### Departments Management
- [ ] **Departments list page**
  - Data table with departments
  - Columns: Name, Code, Head of Department, Sections Count, Staff Count, Actions
  - Search/filter
- [ ] **Create department modal**
  - Form: name, department_code, notes, department_head_id
  - Head of Department dropdown (from employees)
  - Submit to POST /api/1.0/departments
- [ ] **Edit department**
  - Update department details
  - Reassign department head
  - Submit to PUT /api/1.0/departments/:id
- [ ] **View department details**
  - Department info
  - List of sections
  - List of staff members
  - Department statistics

### Grades & Classes Management
- [ ] **Grades list page**
  - Data table with grades (Form 1, Form 2, etc.)
  - Columns: Grade Name, Level, Classes Count, Students Count, Actions
- [ ] **Create grade**
  - Form: name, level, description
  - Submit to POST /api/1.0/grades
- [ ] **Classes list page (per grade)**
  - Data table with classes (1A, 1B, 1C, etc.)
  - Columns: Class Name, Grade, Class Teacher, Students Count, Actions
  - Filter by grade
- [ ] **Create class modal**
  - Form: name, grade_id, class_teacher_id, capacity
  - Class teacher dropdown (from employees)
  - Submit to POST /api/1.0/classes
- [ ] **Edit class**
  - Update class details
  - Reassign class teacher
  - Submit to PUT /api/1.0/classes/:id
- [ ] **View class details**
  - Class info
  - List of students
  - Class teacher info
  - Timetable (future)

### Employees (Staff) Management
- [ ] **Employees list page**
  - Data table with all staff
  - Columns: Name, Employee Number, Position, Department, Section/Class, Email, Phone, Status, Actions
  - Search/filter by name, department, position, status
  - Export to CSV/Excel
- [ ] **Create employee modal/page**
  - Form: firstnames, surname, employee_number, position, gender, email, phone
  - Department & section selection
  - User account creation option (checkbox)
  - If user account: email, password, roles
  - Submit to POST /api/1.0/employees
- [ ] **Edit employee**
  - Update employee details
  - Reassign department/section
  - Manage linked user account
  - Submit to PUT /api/1.0/employees/:id
- [ ] **View employee details**
  - Personal information
  - Department & section
  - Classes taught (if teacher)
  - Linked user account
  - Employment history

---

## Shared Components & Features

### Data Tables
- [ ] **Build reusable table component (TanStack Table)**
  - Sorting
  - Filtering
  - Pagination
  - Column visibility toggle
  - Row selection
  - Bulk actions
  - Export functionality

### Forms & Validation
- [ ] **Create form components**
  - Input fields with validation feedback
  - Select dropdowns (single & multi)
  - Date pickers
  - File upload
  - Form error handling
- [ ] **Implement Zod schemas**
  - User validation schema
  - Role validation schema
  - Department validation schema
  - Employee validation schema
  - Class validation schema

### UI Components
- [ ] **Modals & Dialogs**
  - Confirmation dialogs
  - Form modals
  - Detail view modals
- [ ] **Notifications**
  - Success/error toasts (already using react-hot-toast)
  - Alert banners
  - Inline validation messages
- [ ] **Loading States**
  - Skeleton loaders
  - Spinner components
  - Progress indicators

### Integration & API
- [ ] **API Services**
  - Create auth service (login, logout, refresh)
  - Create users service (CRUD)
  - Create roles service (CRUD)
  - Create departments service (CRUD)
  - Create employees service (CRUD)
  - Create grades/classes service (CRUD)
- [ ] **React Query Integration**
  - Set up queries for data fetching
  - Set up mutations for data updates
  - Implement optimistic updates
  - Cache invalidation strategies
- [ ] **Error Handling**
  - Global error boundary
  - API error handling
  - Validation error display
  - Network error recovery

---

## Testing & Quality

### Testing
- [ ] Unit tests for components
- [ ] Unit tests for hooks
- [ ] Unit tests for services
- [ ] Integration tests for auth flow
- [ ] Integration tests for user management
- [ ] E2E tests for critical flows

### Optimization
- [ ] Code splitting by route
- [ ] Lazy load heavy components
- [ ] Optimize bundle size
- [ ] Performance monitoring
- [ ] Accessibility audit (WCAG 2.1)

---

## Future Enhancements
- [ ] Two-factor authentication (2FA)
- [ ] Single Sign-On (SSO)
- [ ] Advanced audit logging UI
- [ ] User activity dashboard
- [ ] Bulk import users (CSV/Excel)
- [ ] Email notifications for account actions
