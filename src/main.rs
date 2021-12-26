#![no_std]
#![no_main]
#![deny(warnings)]

extern crate cortex_m;
extern crate cortex_m_rt as rt;
extern crate panic_halt;
extern crate rtic;
extern crate stm32g0xx_hal as hal;

mod strip;

use crate::strip::*;
use hal::analog::adc::{SampleTime, VTemp};
use hal::time::*;
use hal::timer::delay::Delay;
use hal::timer::*;
use hal::{analog::adc::Adc, prelude::*};
use hal::{gpio::*, watchdog::IndependedWatchdog};
use hal::{rcc, spi, stm32};
use infrared::{protocols::Nec, PeriodicReceiver};
use smart_leds::SmartLedsWrite;
use ws2812_spi::Ws2812;

pub use defmt_rtt as _;

pub const STRIP_SIZE: usize = 64;
pub const STRIP_FPS: Hertz = Hertz(24);
pub const IR_ADDRESS: u8 = 0;
pub const IR_SAMPLERATE: Hertz = Hertz(20_000);
pub const OVERHEAT_TEMP_RAW: u32 = 1400;

pub type IrPin = gpioa::PA11<Input<Floating>>;
pub type SampleTimer = Timer<stm32::TIM14>;
pub type AnimationTimer = Timer<stm32::TIM16>;
pub type SpiLink = spi::Spi<stm32::SPI1, (spi::NoSck, spi::NoMiso, gpioa::PA12<DefaultMode>)>;

#[rtic::app(device = hal::stm32, peripherals = true)]
mod app {
    use super::*;

    #[shared]
    struct Shared {
        strip: Strip,
    }

    #[local]
    struct Local {
        adc: Adc,
        animation_timer: AnimationTimer,
        delay: Delay<stm32::TIM1>,
        ir: PeriodicReceiver<Nec, IrPin>,
        link: Ws2812<SpiLink>,
        sample_timer: SampleTimer,
        vtemp: VTemp,
        watchdog: IndependedWatchdog,
    }

    #[init]
    fn init(ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        let pll_cfg = rcc::PllConfig::with_hsi(4, 24, 2);
        let rcc_cfg = rcc::Config::pll().pll_cfg(pll_cfg);
        let mut rcc = ctx.device.RCC.freeze(rcc_cfg);

        let port_a = ctx.device.GPIOA.split(&mut rcc);
        let spi = ctx.device.SPI1.spi(
            (spi::NoSck, spi::NoMiso, port_a.pa12),
            spi::MODE_0,
            3.mhz(),
            &mut rcc,
        );

        let delay = ctx.device.TIM1.delay(&mut rcc);

        let mut sample_timer = ctx.device.TIM14.timer(&mut rcc);
        sample_timer.start(IR_SAMPLERATE);
        sample_timer.listen();

        let mut animation_timer = ctx.device.TIM16.timer(&mut rcc);
        animation_timer.start(STRIP_FPS);
        animation_timer.listen();

        let mut adc = ctx.device.ADC.constrain(&mut rcc);
        adc.set_sample_time(SampleTime::T_20);

        let mut vtemp = VTemp::new();
        vtemp.enable(&mut adc);

        let mut watchdog = ctx.device.IWDG.constrain();
        watchdog.start(10.hz());
        let ir = PeriodicReceiver::new(port_a.pa11.into_floating_input(), IR_SAMPLERATE.0);
        let link = Ws2812::new(spi);
        defmt::info!("Init completed");
        (
            Shared {
                strip: Strip::new(),
            },
            Local {
                adc,
                animation_timer,
                delay,
                ir,
                link,
                sample_timer,
                vtemp,
                watchdog,
            },
            init::Monotonics(),
        )
    }

    #[task(binds = TIM14, local = [ir, sample_timer], shared = [strip])]
    fn sample_timer_tick(ctx: sample_timer_tick::Context) {
        ctx.local.sample_timer.clear_irq();
        match ctx.local.ir.poll() {
            Ok(Some(cmd)) if cmd.addr == IR_ADDRESS => {
                defmt::trace!("IR Command: {} {} {}", cmd.addr, cmd.cmd, cmd.repeat);
                let mut strip = ctx.shared.strip;
                strip.lock(|strip| strip.handle_command(cmd.cmd));
            }
            _ => {}
        }
    }

    #[task(binds = TIM16, local = [animation_timer, watchdog], shared = [strip])]
    fn animation_timer_tick(ctx: animation_timer_tick::Context) {
        ctx.local.animation_timer.clear_irq();
        let mut strip = ctx.shared.strip;
        strip.lock(|strip| strip.handle_frame());
        ctx.local.watchdog.feed();
    }

    #[idle(local = [adc,delay,  vtemp, link], shared = [strip])]
    fn idle(ctx: idle::Context) -> ! {
        let mut strip = ctx.shared.strip;
        loop {
            let animation = strip.lock(|strip| strip.animate());
            ctx.local.link.write(animation).ok();
            ctx.local.delay.delay(1.ms());

            let temp_raw: u32 = ctx
                .local
                .adc
                .read(ctx.local.vtemp)
                .expect("temperature read failed");
            if temp_raw > OVERHEAT_TEMP_RAW {
                defmt::info!("Overheat {}", temp_raw);
                strip.lock(|strip| strip.handle_overheat());
            }
        }
    }
}
