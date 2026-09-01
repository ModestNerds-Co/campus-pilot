# Backend Roadmap

## ERP Module Map

Campus Pilot is structured like the multi-module council ERPs this platform draws on: a core/admin platform layer (this file's original content, below) plus one Cargo workspace crate per ERP module under `apis/crates/modules/`.

| Module | Crate | Status | Migration range |
|---|---|---|---|
| Fleet Management | `cp-fleet` | ✅ Implemented (vehicles, drivers) | 010 |
| Vehicle Daily Log | `cp-vehicle-log` | ✅ Implemented (trip sheets against Fleet) | 011 |
| Student Information (SIS) | `cp-sis` | 🚧 Scaffolded | 020s |
| Academics, Gradebook, and Reporting | `cp-academics`, `cp-gradebook`, `cp-academic-reporting` | ✅ Implemented (structure, assessments, mark capture, grading schemes, report cards, progression, transcripts) | 030s, 095-096 |
| Attendance | `cp-attendance` | ✅ Implemented (class registers, learner marks, submit/reopen lifecycle) | 094 |
| Finance & Accounting | `cp-finance` | 🚧 Scaffolded | 040s |
| Fees & Payment Plans | `cp-fees` | 🚧 Scaffolded | 050s |
| HR & Payroll | `cp-hr-payroll` | 🚧 Scaffolded | 060s |
| Procurement & Stores | `cp-procurement` | 🚧 Scaffolded | 070s |
| Library | `cp-library` | 🚧 Scaffolded | 080s |
| Messaging & Comms | `cp-messaging` | 🚧 Scaffolded | 090s |
| Hostel & Boarding | `cp-hostel` | 🚧 Scaffolded | 100s |
| Health & Clinic | `cp-health` | 🚧 Scaffolded | 110s |

"Scaffolded" = the crate, route mount (`/api/1.0/<module>/status`), and client nav entry all exist end-to-end; schema and business logic are future work. See `apis/AGENTS.md` for the workspace layout and the `RequirePermission` mounting rules new modules must follow.

Multi-tenancy foundation (migrations 004-005: `tenants` table, `tenant_id` on every core/module table) landed alongside Fleet — see `apis/AGENTS.md`.

## Setup & Infrastructure
- [x] Initial Rust + Actix Web project setup
- [x] PostgreSQL database connection with SQLx
- [x] Environment configuration (.env)
- [x] Docker configuration
- [x] Server running successfully
- [x] CORS setup (actix-cors)
- [x] Sentry integration for error tracking
- [x] Rate limiting (actix-governor)
- [x] Logging (tracing/env_logger)
- [x] Request/response middleware
- [x] Error handling infrastructure

## Database & Schema
- [x] SQLx setup with compile-time query checking
- [x] Initial migration (001_create_tables.sql)
- [x] System state table with APP_STATE enum
- [x] School profile table (singleton pattern)
- [x] Users table with email, password_hash, roles array
- [x] Event log table for audit trail
- [x] Database triggers and functions
- [ ] **Add comprehensive indexes**
  - Index on users.email
  - Index on users.roles (GIN index for array)
  - Index on employees.employee_number
  - Index on departments.department_code
  - Composite indexes for common queries
- [ ] **Create additional migrations for admin system**
  - User roles table
  - Access rights table (module-based permissions)
  - Routes table (granular permissions)
  - User-role assignment table (many-to-many)
  - Departments table
  - Grades table
  - Classes table
  - Sections table
  - Employees table
  - Password reset tokens table
  - Refresh tokens table
  - User sessions table

---

## Kernel/Bootstrap System
- [x] Kernel service structure
- [x] System state management (Uninitialized → SchoolConfigured → Ready)
- [x] GET /api/1.0/kernel/status endpoint
- [x] POST /api/1.0/kernel/setup-school endpoint
- [x] School configuration DTOs with validation
- [x] KernelDbOps for database operations
- [x] **POST /api/1.0/kernel/setup-admin endpoint**
  - Accept: full_name, email, password, phone
  - Validate password strength (min 10 chars, numbers, symbols)
  - Hash password with bcrypt/argon2
  - Create first admin user with "Super Admin" role
  - Update system state to "Ready"
  - Return success with user info (no password)
- [ ] **Finalize bootstrap flow**
  - Ensure state transitions are atomic
  - Add rollback on failures
  - Prevent re-initialization when Ready

---

## Authentication & Authorization

### Core Authentication
- [x] **Password hashing implementation**
  - Add bcrypt or argon2 crate
  - Create hash_password helper function
  - Create verify_password helper function
  - Configure hash cost/difficulty
- [x] **JWT token implementation**
  - Add jsonwebtoken crate
  - Define JWT claims structure (user_id, email, roles, exp, iat)
  - Create generate_access_token function (15min expiry)
  - Create generate_refresh_token function (7d expiry)
  - Create verify_token function
  - Secret key from environment variable
- [x] **POST /api/1.0/auth/login endpoint**
  - Accept: email (or username), password
  - Validate input (email format, required fields)
  - Query user from database
  - Check if user is active
  - Verify password hash
  - Generate access_token and refresh_token
  - Store refresh_token in database
  - Return: `{ access_token, refresh_token, user: { id, email, full_name, roles }, expires_in }`
  - Handle errors: invalid credentials, inactive user, too many attempts
- [x] **POST /api/1.0/auth/refresh endpoint**
  - Accept: refresh_token
  - Verify refresh token
  - Check token exists in database and not revoked
  - Generate new access_token
  - Optionally rotate refresh_token
  - Return: `{ access_token, refresh_token, expires_in }`
- [x] **POST /api/1.0/auth/logout endpoint**
  - Accept: refresh_token (optional)
  - Revoke refresh token in database
  - Clear user session
  - Return: `{ success: true }`
- [x] **GET /api/1.0/auth/me endpoint**
  - Require authentication
  - Return current user info with roles
  - Include permissions/access_rights

### Authentication Middleware
- [ ] **JWT verification middleware**
  - Extract token from Authorization header (Bearer)
  - Verify token signature and expiration
  - Load user from database
  - Check if user is active
  - Attach user to request context
  - Handle errors: missing token, invalid token, expired token, user not found
- [ ] **Rate limiting for auth endpoints**
  - Limit login attempts (5 per 15min per IP)
  - Limit refresh attempts (10 per 15min per user)
  - Return 429 Too Many Requests on limit

### Password Management
- [ ] **POST /api/1.0/auth/forgot-password endpoint**
  - Accept: email
  - Check if user exists
  - Generate secure reset token (UUID or random string)
  - Store token in password_reset_tokens table (with expiry)
  - Send password reset email (use Lettre)
  - Return: `{ success: true, message: "Reset email sent" }`
- [ ] **POST /api/1.0/auth/reset-password endpoint**
  - Accept: token, new_password
  - Validate token exists and not expired
  - Validate password strength
  - Hash new password
  - Update user password
  - Invalidate reset token
  - Return: `{ success: true }`
- [ ] **POST /api/1.0/auth/change-password endpoint**
  - Require authentication
  - Accept: current_password, new_password
  - Verify current password
  - Hash new password
  - Update user password
  - Invalidate all refresh tokens (force re-login)
  - Return: `{ success: true }`

### Session Management
- [ ] **Create sessions table**
  - user_id, refresh_token, ip_address, user_agent, expires_at, created_at
- [ ] **Track active sessions**
  - Store session on login
  - List user's active sessions
  - Revoke individual sessions
  - Revoke all sessions (except current)

---

## User Management System

### Users CRUD
- [ ] **GET /api/1.0/users endpoint**
  - Require authentication + "user:view" permission
  - Query parameters: page, limit, search, role, department, status, sort
  - Return paginated list of users
  - Columns: id, email, full_name, phone, roles, department, is_active, created_at
  - Support filtering by role, department, status
  - Support search by name/email
- [ ] **GET /api/1.0/users/:id endpoint**
  - Require authentication + "user:view" permission
  - Return user details with roles, department, employee info
  - Include access_rights/permissions
  - Include last_login, created_at, updated_at
- [ ] **POST /api/1.0/users endpoint**
  - Require authentication + "user:create" permission
  - Accept: email, full_name, password, phone, roles[], department_id, section_id, is_active
  - Validate email uniqueness (case-insensitive)
  - Validate password strength
  - Hash password
  - Create user record
  - Assign roles
  - Optionally create linked employee record
  - Return created user (without password)
- [ ] **PUT /api/1.0/users/:id endpoint**
  - Require authentication + "user:edit" permission
  - Accept: email, full_name, phone, roles[], department_id, section_id, is_active
  - Validate email uniqueness (exclude self)
  - Update user record
  - Update role assignments
  - Return updated user
- [ ] **DELETE /api/1.0/users/:id endpoint**
  - Require authentication + "user:delete" permission
  - Soft delete (set deleted_at timestamp)
  - Prevent deleting own account
  - Prevent deleting super admin
  - Revoke all user sessions
  - Return: `{ success: true }`
- [ ] **PUT /api/1.0/users/:id/activate endpoint**
  - Require authentication + "user:edit" permission
  - Set is_active = true
  - Return updated user
- [ ] **PUT /api/1.0/users/:id/deactivate endpoint**
  - Require authentication + "user:edit" permission
  - Set is_active = false
  - Revoke all user sessions
  - Return updated user
- [ ] **POST /api/1.0/users/:id/reset-password endpoint**
  - Require authentication + "user:edit" permission
  - Generate reset token
  - Send password reset email
  - Return: `{ success: true }`
- [ ] **POST /api/1.0/users/:id/force-password-change endpoint**
  - Require authentication + "user:edit" permission
  - Set force_password_change flag
  - User must change password on next login

### User Roles & Permissions
- [ ] **Design role-permission system**
  - Module-based permissions (e.g., "users", "departments", "employees")
  - Permission modes: view, create, edit, delete, full
  - Route-level granular permissions (optional)
  - Many-to-many: users ↔ roles ↔ access_rights ↔ routes
- [ ] **GET /api/1.0/roles endpoint**
  - Require authentication + "role:view" permission
  - Return list of roles with permissions summary
  - Include users_count per role
- [ ] **GET /api/1.0/roles/:id endpoint**
  - Require authentication + "role:view" permission
  - Return role details with all access_rights and routes
- [ ] **POST /api/1.0/roles endpoint**
  - Require authentication + "role:create" permission
  - Accept: name, description
  - Create role record
  - Return created role
- [ ] **PUT /api/1.0/roles/:id endpoint**
  - Require authentication + "role:edit" permission
  - Accept: name, description
  - Update role record
  - Return updated role
- [ ] **DELETE /api/1.0/roles/:id endpoint**
  - Require authentication + "role:delete" permission
  - Check if role is assigned to users
  - Prevent deleting "Super Admin" role
  - Delete role and access_rights cascade
  - Return: `{ success: true }`
- [ ] **POST /api/1.0/roles/:id/permissions endpoint**
  - Require authentication + "role:edit" permission
  - Accept: `{ module, mode, routes[] }`
  - Create or update access_right for role
  - Add routes to access_right
  - Return updated permissions
- [ ] **DELETE /api/1.0/roles/:id/permissions/:access_right_id endpoint**
  - Require authentication + "role:edit" permission
  - Remove specific access_right from role
  - Return: `{ success: true }`

### Permission Middleware
- [ ] **Create permission checking middleware**
  - Check user has required permission for route
  - Support multiple permission formats:
    - Module level: "users:view", "users:edit"
    - Route level: "/api/1.0/users"
  - Return 403 Forbidden if insufficient permissions
  - Log permission denials

---

## School Structure Management

### Departments
- [ ] **Database schema for departments**
  - id, name, department_code (unique), department_head_id (FK to employees), notes, created_at, updated_at, deleted_at
- [ ] **GET /api/1.0/departments endpoint**
  - Return list of departments with head of department info
  - Include sections_count, employees_count
  - Support search and filtering
- [ ] **GET /api/1.0/departments/:id endpoint**
  - Return department details
  - Include list of sections
  - Include list of employees
- [ ] **POST /api/1.0/departments endpoint**
  - Accept: name, department_code, notes, department_head_id
  - Validate department_code uniqueness
  - Create department
- [ ] **PUT /api/1.0/departments/:id endpoint**
  - Update department details
  - Handle department head reassignment
- [ ] **DELETE /api/1.0/departments/:id endpoint**
  - Soft delete department
  - Check for dependencies (employees, sections)

### Grades & Classes
- [ ] **Database schema for grades**
  - id, name (e.g., "Form 1"), level (1-6), description, created_at, updated_at
- [ ] **Database schema for classes**
  - id, name (e.g., "1A"), grade_id (FK), class_teacher_id (FK to employees), capacity, created_at, updated_at, deleted_at
- [ ] **GET /api/1.0/grades endpoint**
  - Return list of grades with classes_count
- [ ] **POST /api/1.0/grades endpoint**
  - Accept: name, level, description
  - Create grade
- [ ] **GET /api/1.0/classes endpoint**
  - Return list of classes with grade, teacher, students_count
  - Filter by grade_id
- [ ] **POST /api/1.0/classes endpoint**
  - Accept: name, grade_id, class_teacher_id, capacity
  - Create class
- [ ] **PUT /api/1.0/classes/:id endpoint**
  - Update class details
  - Handle teacher reassignment

### Sections (Sub-departments)
- [ ] **Database schema for sections**
  - id, name, department_id (FK), section_head_id (FK to employees), created_at, updated_at, deleted_at
- [ ] **GET /api/1.0/sections endpoint**
  - Return sections list
  - Filter by department_id
- [ ] **POST /api/1.0/sections endpoint**
  - Accept: name, department_id, section_head_id
  - Create section
- [ ] **PUT /api/1.0/sections/:id endpoint**
  - Update section
- [ ] **DELETE /api/1.0/sections/:id endpoint**
  - Soft delete section

### Employees (Staff)
- [ ] **Database schema for employees**
  - id, employee_number (unique), firstnames, surname, position, gender, email, phone, department_id (FK), section_id (FK), user_id (FK, nullable), created_at, updated_at, deleted_at
- [ ] **GET /api/1.0/employees endpoint**
  - Return paginated employees list
  - Include department, section, user account info
  - Support search, filter by department, position, status
  - Export to CSV
- [ ] **GET /api/1.0/employees/:id endpoint**
  - Return employee details
  - Include user account if linked
  - Include classes taught (if teacher)
- [ ] **POST /api/1.0/employees endpoint**
  - Accept: employee_number, firstnames, surname, position, gender, email, phone, department_id, section_id
  - Optional: create_user_account, user_email, user_password, user_roles[]
  - Validate employee_number uniqueness
  - Create employee record
  - Optionally create linked user account
  - Return created employee
- [ ] **PUT /api/1.0/employees/:id endpoint**
  - Update employee details
  - Handle department/section reassignment
- [ ] **DELETE /api/1.0/employees/:id endpoint**
  - Soft delete employee
  - Optionally deactivate linked user account

---

## Audit & Logging

### Audit Trail
- [x] Event log table (basic structure exists)
- [ ] **Enhance event logging**
  - Log all user management actions (create, update, delete, activate, deactivate)
  - Log role and permission changes
  - Log authentication events (login, logout, failed attempts)
  - Store: user_id, action, table_name, record_id, old_value, new_value, ip_address, user_agent, timestamp
- [ ] **GET /api/1.0/audit-logs endpoint**
  - Require authentication + "audit:view" permission
  - Return paginated audit logs
  - Filter by user, action, table, date range
  - Export to CSV

### Activity Tracking
- [ ] **Track user activity**
  - Last login timestamp
  - Login history (IP, user_agent, timestamp)
  - Failed login attempts
- [ ] **GET /api/1.0/users/:id/activity endpoint**
  - Return user activity log
  - Login history
  - Actions performed
  - Sessions

---

## Email System

### Email Infrastructure
- [x] Lettre email library integrated
- [ ] **Email templates system**
  - Welcome email template
  - Password reset email template
  - Password changed notification
  - Account created notification
  - Account deactivated notification
- [ ] **Email service implementation**
  - send_welcome_email(user)
  - send_password_reset_email(user, token)
  - send_password_changed_email(user)
  - send_account_created_email(user, temp_password)
- [ ] **Email queue (optional)**
  - Background job processing
  - Retry failed emails
  - Email delivery tracking

---

## Security Enhancements

### Security Hardening
- [x] Rate limiting (actix-governor)
- [x] CORS configuration
- [x] Input validation (validator crate)
- [ ] **Security headers middleware**
  - X-Content-Type-Options: nosniff
  - X-Frame-Options: DENY
  - X-XSS-Protection: 1; mode=block
  - Strict-Transport-Security (HSTS)
  - Content-Security-Policy (CSP)
- [ ] **Request size limits**
  - Limit JSON payload size
  - Limit file upload size
- [ ] **SQL injection protection**
  - Verify all SQLx queries use parameterization
  - No dynamic SQL string concatenation
- [ ] **CSRF protection** (if using cookies)
  - CSRF token generation
  - Validate token on state-changing requests
- [ ] **Brute force protection**
  - Lock account after N failed login attempts
  - Temporary IP blocking
  - CAPTCHA on repeated failures

---

## API Documentation & Standards

### API Documentation
- [ ] **OpenAPI/Swagger integration**
  - Generate OpenAPI 3.0 spec
  - Document all endpoints with request/response schemas
  - Add authentication requirements
  - Add examples
- [ ] **API versioning strategy**
  - Current: /api/1.0/
  - Plan for future versions

### Response Standardization
- [x] ApiResponse wrapper type
- [x] Standardized error responses
- [ ] **Pagination helpers**
  - Consistent pagination format
  - Page, limit, total, total_pages
- [ ] **Filtering and sorting helpers**
  - Standardized query params
  - Dynamic filter builder

---

## Testing & Quality

### Testing
- [ ] **Unit tests**
  - Password hashing/verification
  - JWT token generation/verification
  - Permission checking logic
  - Business logic functions
- [ ] **Integration tests**
  - Auth flow (login, refresh, logout)
  - User CRUD operations
  - Role and permission management
  - Department/Employee management
- [ ] **Database tests**
  - Transaction rollback tests
  - Constraint validation
  - Soft delete behavior
- [ ] **Load testing**
  - Auth endpoints under load
  - Concurrent user operations
  - Database connection pooling

### Optimization
- [ ] **Query optimization**
  - Add missing indexes
  - Optimize N+1 queries with eager loading
  - Use database views for complex queries
- [ ] **Connection pooling**
  - Tune SQLx pool size
  - Monitor connection usage
- [ ] **Caching strategy**
  - Redis integration
  - Cache user permissions
  - Cache frequently accessed data

---

## Future Enhancements
- [ ] Two-factor authentication (TOTP)
- [ ] Single Sign-On (SSO) integration
- [ ] OAuth2 server implementation
- [ ] WebSocket for real-time notifications
- [ ] GraphQL API (alternative to REST)
- [ ] Bulk operations (import users, employees from CSV)
- [ ] Advanced reporting endpoints
- [ ] File attachment system for user profiles
