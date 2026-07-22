FROM python:3.14-alpine

COPY requirements.txt /tmp/requirements.txt
RUN pip install --no-cache-dir -r /tmp/requirements.txt

WORKDIR /app
COPY worker.py /app/worker.py

EXPOSE 8090
CMD ["uvicorn", "worker:app", "--host", "0.0.0.0", "--port", "8090"]
