# mailcrab

![Version: 0.2.2](https://img.shields.io/badge/Version-0.2.2-informational?style=flat-square) ![Type: application](https://img.shields.io/badge/Type-application-informational?style=flat-square) ![AppVersion: 1.11.0](https://img.shields.io/badge/AppVersion-1.11.0-informational?style=flat-square)

A Helm chart for deploying MailCrab in Kubernetes.

## Values

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| affinity | object | `{}` | Affinity rules for pod assignment. |
| autoscaling.enabled | bool | `false` | Enable autoscaling using a HorizontalPodAutoscaler. When enabled, `replicaCount` is ignored. |
| autoscaling.minReplicas | int | `1` | Minimum number of replicas when autoscaling is enabled. |
| autoscaling.maxReplicas | int | `5` | Maximum number of replicas when autoscaling is enabled. |
| autoscaling.targetCPUUtilizationPercentage | int | `80` | Target average CPU utilization percentage. Set to `null` to disable. |
| autoscaling.targetMemoryUtilizationPercentage | int | `nil` | Target average memory utilization percentage. Set to `null` to disable. |
| env | list | `[]` | Extra environment variables to pass to the MailCrab container. See https://github.com/tweedegolf/mailcrab#configuration |
| fullnameOverride | string | `""` | Configure the fullname override for resources. |
| image.pullPolicy | string | `"Always"` | Specify an imagePullPolicy, defaults to 'Always' if image tag is 'latest', else set to 'IfNotPresent' |
| image.repository | string | `"docker.io/marlonb/mailcrab"` | Image to use for the deployment. |
| image.tag | string | `"latest"` | Overrides the image tag whose default is the chart appVersion. |
| imagePullSecrets | list | `[]` | If needed, specify custom imagePullSecrets to use with private registries. |
| ingress.annotations | object | `{}` | Annotations to add to the ingress |
| ingress.className | string | `""` | The class of the Ingress controller to use (e.g. nginx, traefik, haproxy). Leave empty to use the cluster default. |
| ingress.enabled | bool | `false` | Enables the use of an ingress controller. |
| ingress.hosts[0].host | string | `"chart-example.local"` | The host to use for the ingress. |
| ingress.hosts[0].paths[0].path | string | `"/"` | The path to use for the ingress. |
| ingress.hosts[0].paths[0].pathType | string | `"ImplementationSpecific"` |  |
| ingress.tls | list | `[]` | TLS configuration for ingress |
| livenessProbe | object | `{"httpGet":{"path":"/","port":"http"}}` | Configure the liveness probe for the container. |
| nameOverride | string | `""` | Configure the name override for resources. |
| nodeSelector | object | `{}` | Node labels for pod assignment. |
| podAnnotations | object | `{}` | Annotations to add to the pods. |
| podLabels | object | `{}` | Additional labels to add to the pods. |
| podSecurityContext | object | `{}` | Configure the pod security context. |
| priorityClassName | string | `""` | Name of an existing PriorityClass to assign to the pods. |
| readinessProbe | object | `{"httpGet":{"path":"/","port":"http"}}` | Configure the readiness probe for the container. |
| replicaCount | int | `1` | Configure the number of replicas to run. Ignored when `autoscaling.enabled` is true. |
| resources | object | `{}` | Resource requests and limits for the container. |
| securityContext | object | `{}` | Configure the security context for the container. |
| service.containerPort | int | `1080` | The container port the web interface listens on (sets the HTTP_PORT env var). |
| service.port | int | `80` | The port to expose on the service for the web interface. |
| service.smtpPort | int | `1025` | The container/service port the SMTP server listens on (sets the SMTP_PORT env var). |
| service.type | string | `"ClusterIP"` | The type of service to create. |
| serviceAccount.annotations | object | `{}` | Annotations to add to the service account |
| serviceAccount.create | bool | `true` | Specifies whether a service account should be created |
| serviceAccount.name | string | `""` | The name of the service account to use. If not set and create is true, a name is generated using the fullname template |
| tolerations | list | `[]` | Tolerations for pod assignment. |
