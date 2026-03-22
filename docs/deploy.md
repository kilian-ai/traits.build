## Deploy to Fly
Based on the project setup, here are the steps to deploy from local to your-fly-app.fly.dev:
```sh
cd path/to/traits.build

# 1. Build the Docker image for amd64 (Fly runs amd64, Mac is aarch64)
docker buildx build --platform linux/amd64 -t registry.fly.io/<your-fly-app>:latest .

# 2. Deploy to Fly.io
fly deploy --now --local-only --image registry.fly.io/<your-fly-app>:latest

# 3. If machine auto-stops, start it
fly machines start 683039db395208
```
Or if the Fast Deploy button on the admin dashboard is working, that pushes just the binary to /data/traits on the persistent volume (faster, no Docker rebuild needed). But the admin page itself needs a full Docker deploy since the HTML is compiled into the binary.

Alternatively, the admin dashboard has a "Full Deploy" button that runs www.admin.deploy which does a restart of the existing image — but that won't pick up new code. For new code, you need the Docker build + fly deploy path above.