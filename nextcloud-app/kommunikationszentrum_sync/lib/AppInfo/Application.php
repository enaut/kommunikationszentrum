<?php

declare(strict_types=1);

namespace OCA\KommunikationszentrumSync\AppInfo;

use OCA\KommunikationszentrumSync\Listener\UserEventListener;
use OCP\Accounts\UserUpdatedEvent;
use OCP\AppFramework\App;
use OCP\AppFramework\Bootstrap\IBootContext;
use OCP\AppFramework\Bootstrap\IBootstrap;
use OCP\AppFramework\Bootstrap\IRegistrationContext;
use OCP\User\Events\UserChangedEvent;
use OCP\User\Events\UserCreatedEvent;
use OCP\User\Events\UserDeletedEvent;

class Application extends App implements IBootstrap {
    public const APP_ID = 'kommunikationszentrum_sync';

    public function __construct(array $params = []) {
        parent::__construct(self::APP_ID, $params);
    }

    public function register(IRegistrationContext $context): void {
        $context->registerEventListener(UserCreatedEvent::class, UserEventListener::class);
        $context->registerEventListener(UserChangedEvent::class, UserEventListener::class);
        $context->registerEventListener(UserDeletedEvent::class, UserEventListener::class);
        $context->registerEventListener(UserUpdatedEvent::class, UserEventListener::class);
    }

    public function boot(IBootContext $context): void {
    }
}
