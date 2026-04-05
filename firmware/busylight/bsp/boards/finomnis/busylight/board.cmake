board_runner_args(jlink "--device=STM32U073CC" "--reset-after-load")

include(${ZEPHYR_BASE}/boards/common/jlink.board.cmake)
