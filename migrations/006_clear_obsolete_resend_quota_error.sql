UPDATE providers
SET last_error = NULL
WHERE last_error LIKE '%x-resend-monthly-quota%';
