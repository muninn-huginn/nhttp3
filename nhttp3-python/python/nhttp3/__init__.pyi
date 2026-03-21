"""Type stubs for nhttp3."""

__version__: str

class Config:
    max_idle_timeout: float
    initial_max_data: int
    initial_max_stream_data_bidi_local: int
    initial_max_stream_data_bidi_remote: int
    initial_max_stream_data_uni: int
    initial_max_streams_bidi: int
    initial_max_streams_uni: int
    enable_0rtt: bool

    def __init__(self) -> None: ...
    def __repr__(self) -> str: ...
