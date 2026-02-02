{{/*
Expand the name of the chart.
*/}}
{{- define "synstack.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "synstack.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "synstack.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "synstack.labels" -}}
helm.sh/chart: {{ include "synstack.chart" . }}
{{ include "synstack.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "synstack.selectorLabels" -}}
app.kubernetes.io/name: {{ include "synstack.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
API labels
*/}}
{{- define "synstack.api.labels" -}}
{{ include "synstack.labels" . }}
app.kubernetes.io/component: api
{{- end }}

{{/*
API selector labels
*/}}
{{- define "synstack.api.selectorLabels" -}}
{{ include "synstack.selectorLabels" . }}
app.kubernetes.io/component: api
{{- end }}

{{/*
PostgreSQL host
*/}}
{{- define "synstack.postgresql.host" -}}
{{- if .Values.postgresql.enabled }}
{{- printf "%s-postgresql" .Release.Name }}
{{- else }}
{{- .Values.externalDatabase.host }}
{{- end }}
{{- end }}

{{/*
Database URL
*/}}
{{- define "synstack.databaseUrl" -}}
{{- $host := include "synstack.postgresql.host" . }}
{{- $user := .Values.postgresql.auth.username }}
{{- $db := .Values.postgresql.auth.database }}
{{- printf "postgres://%s:$(DATABASE_PASSWORD)@%s:5432/%s" $user $host $db }}
{{- end }}

{{/*
Gitea URL
*/}}
{{- define "synstack.giteaUrl" -}}
{{- if .Values.gitea.enabled }}
{{- printf "http://%s-gitea-http:3000" .Release.Name }}
{{- else }}
{{- .Values.externalGitea.url }}
{{- end }}
{{- end }}
