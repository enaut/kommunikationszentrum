<?php

declare(strict_types=1);

namespace OCA\KommunikationszentrumSync\Listener;

use OCA\KommunikationszentrumSync\AppInfo\Application;
use OCP\Accounts\UserUpdatedEvent;
use OCP\EventDispatcher\Event;
use OCP\EventDispatcher\IEventListener;
use OCP\Http\Client\IClientService;
use OCP\IConfig;
use OCP\User\Events\UserChangedEvent;
use OCP\User\Events\UserCreatedEvent;
use OCP\User\Events\UserDeletedEvent;
use Psr\Log\LoggerInterface;

class UserEventListener implements IEventListener {
    public function __construct(
        private IConfig $config,
        private IClientService $clientService,
        private LoggerInterface $logger,
    ) {}

    public function handle(Event $event): void {
        if (
            !($event instanceof UserCreatedEvent)
            && !($event instanceof UserChangedEvent)
            && !($event instanceof UserDeletedEvent)
            && !($event instanceof UserUpdatedEvent)
        ) {
            return;
        }

        $webhookUrl = trim($this->config->getAppValue(Application::APP_ID, 'webhook_url', ''));
        $webhookSecret = trim($this->config->getAppValue(Application::APP_ID, 'webhook_secret', ''));

        if (empty($webhookUrl)) {
            $this->logger->debug('Kommunikationszentrum sync: webhook_url is not configured, skipping event', [
                'app' => Application::APP_ID,
            ]);
            return;
        }

        try {
            $user = $event->getUser();
            $uid = $user ? $user->getUID() : null;

            if (empty($uid)) {
                $this->logger->warning('User event received without valid UID', [
                    'app' => Application::APP_ID,
                    'event' => get_class($event),
                ]);
                return;
            }

            $eventClass = get_class($event);
            $payload = [
                'event' => $eventClass,
                'uid' => $uid,
                'user' => [
                    'uid' => $uid,
                ],
            ];

            if ($event instanceof UserChangedEvent) {
                $payload['feature'] = $event->getFeature();
            }

            $client = $this->clientService->newClient();
            $options = [
                'headers' => [
                    'Content-Type' => 'application/json',
                    'X-Nextcloud-Token' => $webhookSecret,
                ],
                'body' => json_encode($payload, JSON_THROW_ON_ERROR),
                'timeout' => 3,
                'connect_timeout' => 2,
            ];

            $response = $client->post($webhookUrl, $options);
            $statusCode = $response->getStatusCode();

            if ($statusCode >= 200 && $statusCode < 300) {
                $this->logger->info('Dispatched user sync webhook for {uid} ({event}) - HTTP {status}', [
                    'app' => Application::APP_ID,
                    'uid' => $uid,
                    'event' => $eventClass,
                    'status' => $statusCode,
                ]);
            } else {
                $this->logger->warning('Failed to dispatch user sync webhook for {uid} - HTTP {status}', [
                    'app' => Application::APP_ID,
                    'uid' => $uid,
                    'status' => $statusCode,
                ]);
            }
        } catch (\Throwable $e) {
            // Safety: Never block or fail Nextcloud core user operations if sync service is down
            $this->logger->error('Error dispatching user sync webhook: {message}', [
                'app' => Application::APP_ID,
                'message' => $e->getMessage(),
                'exception' => $e,
            ]);
        }
    }
}
