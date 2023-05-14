# Reverse proxy guide

If you want to put MailCrab behind a reverse proxy, you can use the following configurations:

## Nginx

```nginx
server {
    listen 80;

    server_name <your server name>;

    location / {
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_pass http://<your MailCrab server>:1080;
    }

    location /ws {
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_pass http://<your MailCrab server>:1080;
    }
}
```

If you are using `MAILCRAB_PREFIX`, for example `MAILCRAB_PREFIX=emails`:

```nginx
location /emails {
    ...
}

location /emails/ws {
    ...
}
```

## Apache2
- For apache2 version 2.4.47 and later:

  Enable `mod_proxy` and `mod_proxy_http`. Then you can use the following snippet:

  ```apache
  <VirtualHost *:80>
      ProxyPass "/ws" "http://<your MailCrab server>:1080/ws" upgrade=websocket

      ProxyPass "/" "http://<your MailCrab server>:1080/"
      ProxyPassReverse "/" "http://<your MailCrab server>:1080/"
  </VirtualHost>
  ```
  
  If you are using `MAILCRAB_PREFIX`, for example `MAILCRAB_PREFIX=emails`:

  ```apache
  <VirtualHost *:80>
      ProxyPass "/emails/ws" "http://<your MailCrab server>:1080/emails/ws" upgrade=websocket

      ProxyPass "/emails" "http://<your MailCrab server>:1080/emails"
      ProxyPassReverse "/emails" "http://<your MailCrab server>:1080/emails"
  </VirtualHost>
  ```

- For apache2 version 2.4.46 and earlier:

  Enable `mod_proxy`, `mod_proxy_http` and `mod_proxy_wstunnel`. Then you can use the following snippet:

  ```apache
  <VirtualHost *:80>
      ProxyPass "/ws" "ws://<your MailCrab server>:1080/ws"

      ProxyPass "/" "http://<your MailCrab server>:1080/"
      ProxyPassReverse "/" "http://<your MailCrab server>:1080/"
  </VirtualHost>
  ```

  If you are using `MAILCRAB_PREFIX`, for example `MAILCRAB_PREFIX=emails`:

  ```apache
  <VirtualHost *:80>
      ProxyPass "/emails/ws" "ws://<your MailCrab server>:1080/emails/ws"

      ProxyPass "/emails" "http://<your MailCrab server>:1080/emails"
      ProxyPassReverse "/emails" "http://<your MailCrab server>:1080/emails"
  </VirtualHost>
  ```

## Other reverse proxies

MailCrab is using 3 endpoints in its backend:

- `/api` for the messages.
- `/ws` for client-backend communication (using WebSocket).
- `/static` for assets.

Just proxy those 3 endpoints to the backend and it should work properly.

If you are using `MAILCRAB_PREFIX`, add the prefix before each endpoint: `/<prefix>/{api, ws, static}`
