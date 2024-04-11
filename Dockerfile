FROM python:3.11-slim-buster

# set work directory
WORKDIR /app

# Add the wait script to the image
ADD https://github.com/ufoscout/docker-compose-wait/releases/download/2.9.0/wait  /wait
RUN chmod +x /wait

# set environment variables
ENV PYTHONDONTWRITEBYTECODE 1
ENV PYTHONUNBUFFERED 1

# copy project dependency files
COPY poetry.lock pyproject.toml /app/

# create virtual environment & activate it
RUN python -m venv env
RUN . env/bin/activate

# install dependencies
RUN pip install --upgrade pip poetry
RUN poetry install

# copy project
COPY . /app/

# run entrypoint.sh
ENTRYPOINT ["/app/app.sh"]

