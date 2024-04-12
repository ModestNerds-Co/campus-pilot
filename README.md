# CampusPilot: School Management System

## Overview
CampusPilot is a comprehensive school management system designed to streamline administrative tasks, enhance communication, and foster academic excellence in educational institutions. Built with Django and Django REST framework, and powered by PostgreSQL, CampusPilot offers robust, scalable, and flexible solutions to meet the diverse needs of schools and colleges.

## Key Features
- **Student Information System (SIS)**: Manage student profiles, grades, attendance, and more, all in one place.
- **Faculty and Staff Management**: Keep track of faculty and staff details, schedules, and payroll.
- **Course and Curriculum Management**: Plan and manage academic curricula, schedules, and resources efficiently.
- **Financial Management**: Handle billing, fees, and financial aid seamlessly.
- **Communication Portal**: Facilitate effective communication among students, teachers, and parents.
- **Analytics and Reporting**: Generate insightful reports and analytics to aid decision-making.

## Getting Started

### Prerequisites
- Python 3.8 or higher
- Django 4.0 or higher
- Django REST framework
- PostgreSQL 14 or higher

### Installation
1. Clone the repository:
```bash
$ git clone git@github.com:ModestNerds-Co/campus-pilot-apis.git
$ cd campus-pilot-apis
```
   
2. Install the required dependencies:
```bash
$ poetry install
```

3. Run the migrations to create the database schema:
 ```bash
$ python manage.py makemigrations
$ python manage.py migrate
```
   
4. Start the development server:
```bash
$ python manage.py runserver
```

## Usage
Navigate to `http://127.0.0.1:8000/` in your web browser to access the CampusPilot application. Use the admin interface at `http://127.0.0.1:8000/admin` to manage the system.

## Contributing
We welcome contributions to CampusPilot! If you have suggestions or improvements, please fork the repo and submit a pull request.

---

## License
CampusPilot is licensed under a [Proprietary & Confidential License](LICENSE).
