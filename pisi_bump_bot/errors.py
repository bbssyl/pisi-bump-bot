class BotError(Exception):
    pass


class FetchError(BotError):
    pass


class RecipesUnavailableError(BotError):
    pass


class UpstreamError(BotError):
    pass


class RateLimitExceeded(BotError):
    pass
