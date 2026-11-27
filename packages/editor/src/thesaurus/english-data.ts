// avoid-ai-design-ignore-file: CP2, M4 (a word list: it names words, it does not use them)
// Docxtor's English thesaurus data: a compact list of groups of words that mean about the same, written for Docxtor
// and released with it under the same licences (Apache-2.0 or MIT). It is not WordNet or any other list copied
// from elsewhere, so there is nothing to attribute and nothing to keep in step with an outside licence.
//
// It is small on purpose (a few hundred groups, built to be loaded the first time someone asks for synonyms, not when
// the editor starts). A word that is in several groups gets one meaning for each. A bigger list belongs in an extension
// of its own: `ctx.thesaurus.register` takes any source.
//
// Format, one group a line: `pos|word,word,word`. `pos` is n (noun), v (verb), a (adjective) or r (adverb). The first word
// names the group. A line that starts with `!` lists opposites: `!a|big,large|small,little` says each word of the first
// list is the opposite of each word of the second.
export const ENGLISH_GROUPS = `
a|big,large,huge,great,vast,enormous,immense,giant,massive,substantial,sizable,considerable,hefty,bulky
a|huge,enormous,gigantic,colossal,massive,immense,vast,mammoth,towering,monumental,tremendous
a|small,little,tiny,minute,compact,petite,slight,modest,miniature,diminutive,wee,minor
a|tiny,minuscule,microscopic,miniature,minute,infinitesimal,teeny
a|good,fine,excellent,great,superb,splendid,wonderful,positive,favourable,pleasant,decent,sound
a|excellent,outstanding,superb,exceptional,first-rate,brilliant,superior,magnificent,terrific,remarkable
a|bad,poor,awful,terrible,dreadful,inferior,unpleasant,harmful,unsatisfactory,substandard,lousy
a|terrible,awful,dreadful,horrible,appalling,atrocious,ghastly,shocking,dire
a|happy,glad,cheerful,joyful,delighted,pleased,content,merry,jolly,thrilled,elated,upbeat
a|sad,unhappy,sorrowful,gloomy,miserable,downcast,depressed,melancholy,dejected,glum,blue,heartbroken
a|angry,mad,furious,irate,annoyed,cross,enraged,livid,indignant,irritated,incensed,outraged
a|afraid,scared,frightened,fearful,terrified,anxious,alarmed,nervous,apprehensive,petrified
a|nervous,anxious,tense,uneasy,jittery,edgy,apprehensive,worried,restless
a|tired,weary,exhausted,fatigued,drained,sleepy,worn out,spent,drowsy
a|clever,smart,intelligent,bright,sharp,brilliant,gifted,quick,wise,astute,shrewd,able
a|stupid,foolish,silly,dim,dull,ignorant,unwise,thoughtless,brainless,absurd
a|wise,sensible,prudent,judicious,sage,thoughtful,discerning,shrewd,informed
a|fast,quick,rapid,swift,speedy,brisk,hasty,prompt,fleet,express
a|slow,sluggish,gradual,unhurried,leisurely,plodding,delayed,tardy,measured
a|easy,simple,effortless,straightforward,painless,basic,uncomplicated,smooth,elementary
a|difficult,hard,tough,challenging,demanding,tricky,complicated,arduous,strenuous,laborious,taxing
a|hard,firm,solid,rigid,stiff,tough,dense,unyielding,compact
a|soft,gentle,smooth,tender,delicate,fluffy,supple,mild,downy,velvety
a|important,significant,essential,vital,crucial,critical,key,major,central,fundamental,principal,necessary
a|unimportant,trivial,minor,insignificant,petty,negligible,irrelevant,slight,inconsequential
a|necessary,essential,required,needed,vital,compulsory,mandatory,obligatory,indispensable,imperative
a|main,chief,principal,primary,leading,foremost,central,prime,major,key,dominant
a|new,fresh,novel,modern,recent,original,current,latest,innovative,contemporary,up-to-date
a|old,aged,ancient,elderly,mature,antique,vintage,archaic,outdated,obsolete,veteran
a|modern,contemporary,current,present-day,up-to-date,recent,latest,fashionable,advanced
a|beautiful,pretty,lovely,attractive,gorgeous,handsome,stunning,elegant,charming,appealing,striking
a|ugly,unattractive,unsightly,hideous,plain,unlovely,repulsive,grotesque
a|strong,powerful,sturdy,robust,tough,mighty,forceful,solid,durable,potent,vigorous
a|weak,feeble,frail,fragile,delicate,flimsy,faint,powerless,infirm,limp
a|rich,wealthy,affluent,prosperous,well-off,well-to-do,loaded,opulent,moneyed
a|poor,needy,destitute,impoverished,penniless,broke,hard up,deprived,disadvantaged
a|cheap,inexpensive,affordable,economical,low-cost,budget,reasonable,cut-price
a|expensive,costly,pricey,dear,lavish,extravagant,exorbitant,high-priced
a|hot,warm,heated,boiling,scorching,sweltering,burning,torrid,fiery,sizzling
a|cold,chilly,cool,freezing,icy,frosty,frigid,bitter,wintry,nippy
a|wet,damp,moist,soaked,drenched,soggy,humid,sodden,saturated,dripping
a|dry,arid,parched,dehydrated,barren,thirsty,bone-dry,withered
a|clean,spotless,tidy,pure,fresh,immaculate,hygienic,neat,sanitary
a|dirty,filthy,grubby,soiled,messy,grimy,unclean,stained,muddy,squalid
a|quiet,silent,hushed,still,peaceful,calm,noiseless,soft,muted,tranquil
a|loud,noisy,deafening,booming,thunderous,blaring,raucous,rowdy,ear-splitting
a|calm,peaceful,tranquil,serene,placid,relaxed,composed,still,quiet,untroubled,collected
a|brave,courageous,bold,fearless,daring,valiant,heroic,gallant,plucky,intrepid
a|honest,truthful,sincere,candid,frank,genuine,straightforward,open,trustworthy,upright
a|kind,caring,gentle,generous,considerate,thoughtful,benevolent,compassionate,warm,gracious
a|cruel,harsh,brutal,merciless,ruthless,savage,heartless,callous,vicious,unkind
a|rude,impolite,discourteous,disrespectful,insolent,curt,abrupt,uncivil,offensive
a|polite,courteous,respectful,civil,well-mannered,gracious,mannerly,refined
a|funny,amusing,humorous,comical,hilarious,witty,entertaining,droll,laughable
a|strange,odd,peculiar,weird,unusual,curious,bizarre,queer,eccentric,uncommon,unfamiliar
a|unusual,uncommon,rare,exceptional,atypical,unfamiliar,extraordinary,singular,remarkable
a|common,usual,ordinary,typical,regular,normal,familiar,frequent,everyday,widespread,standard
a|normal,usual,standard,regular,ordinary,typical,average,conventional,routine
a|rare,scarce,uncommon,infrequent,unusual,sparse,exceptional,few
a|certain,sure,definite,positive,confident,convinced,assured,undoubted,guaranteed
a|possible,feasible,conceivable,potential,viable,achievable,likely,plausible,practicable
a|likely,probable,expected,plausible,promising,credible,apt,liable
a|obvious,clear,evident,apparent,plain,patent,visible,noticeable,unmistakable,manifest
a|clear,plain,obvious,transparent,evident,lucid,distinct,understandable,coherent,crisp
a|complete,whole,entire,full,total,intact,finished,thorough,absolute,comprehensive
a|whole,entire,complete,full,total,intact,undivided,unbroken
a|full,filled,packed,crowded,loaded,stuffed,brimming,replete,crammed,complete
a|empty,vacant,bare,hollow,blank,unoccupied,void,deserted,barren
a|real,genuine,authentic,actual,true,factual,legitimate,bona fide,veritable
a|false,untrue,wrong,incorrect,mistaken,fake,bogus,fictitious,erroneous,spurious,inaccurate
a|fake,false,counterfeit,forged,sham,bogus,phoney,imitation,artificial,mock
a|true,correct,accurate,right,factual,exact,valid,genuine,truthful
a|correct,right,accurate,exact,precise,proper,true,faultless,valid
a|wrong,incorrect,mistaken,inaccurate,erroneous,false,faulty,flawed,untrue
a|exact,precise,accurate,specific,correct,definite,strict,careful,meticulous
a|careful,cautious,wary,prudent,attentive,watchful,thorough,meticulous,painstaking,vigilant,diligent
a|careless,sloppy,negligent,reckless,thoughtless,slapdash,heedless,lax,hasty
a|busy,occupied,engaged,active,employed,hectic,industrious,tied up,swamped
a|lazy,idle,slothful,sluggish,indolent,lethargic,inactive,slack,work-shy
a|hard-working,diligent,industrious,conscientious,dedicated,tireless,assiduous,persevering
a|proud,arrogant,conceited,haughty,vain,boastful,smug,self-satisfied,stuck-up
a|shy,timid,bashful,reserved,reticent,retiring,self-conscious,diffident,demure
a|friendly,amiable,kind,pleasant,sociable,genial,warm,approachable,affable,cordial,outgoing
a|hostile,unfriendly,antagonistic,aggressive,belligerent,opposed,adverse,cold
a|famous,well-known,renowned,celebrated,noted,eminent,prominent,notable,legendary,distinguished
a|ordinary,common,usual,plain,average,everyday,regular,routine,humdrum,mundane,unremarkable
a|special,particular,unique,exceptional,distinctive,specific,singular,notable,extraordinary
a|different,distinct,diverse,unlike,various,separate,dissimilar,contrasting,varied,disparate
a|similar,alike,comparable,akin,analogous,like,related,parallel,resembling,corresponding
a|same,identical,equal,equivalent,matching,alike,uniform,unchanged,twin
a|equal,even,identical,equivalent,balanced,matching,uniform,fair,level
a|enough,sufficient,adequate,ample,plenty,satisfactory,abundant
a|many,numerous,several,various,countless,multiple,abundant,plentiful,innumerable,manifold
a|few,scarce,rare,sparse,limited,meagre,sporadic,infrequent
a|whole,total,entire,complete,overall,aggregate,gross,sum
a|extra,additional,more,further,added,supplementary,spare,auxiliary,surplus
a|main,principal,primary,key,cardinal,central,leading
a|basic,fundamental,essential,elementary,primary,core,underlying,simple,rudimentary
a|serious,grave,severe,solemn,earnest,critical,weighty,acute,sober
a|severe,harsh,strict,stern,serious,intense,extreme,rigorous,grave,acute,austere
a|extreme,intense,severe,excessive,radical,drastic,utmost,acute,exceptional
a|sudden,abrupt,unexpected,swift,rapid,sharp,unforeseen,hasty,instant
a|ready,prepared,set,primed,willing,available,equipped,fit,ripe
a|available,accessible,obtainable,free,handy,on hand,usable,at hand,vacant
a|dangerous,risky,hazardous,perilous,unsafe,treacherous,threatening,precarious,harmful
a|safe,secure,protected,sheltered,harmless,unharmed,reliable,sound,guarded
a|useful,helpful,practical,valuable,beneficial,handy,functional,effective,worthwhile,advantageous
a|useless,pointless,worthless,futile,ineffective,vain,unproductive,fruitless,hopeless
a|effective,successful,efficient,productive,powerful,capable,potent,influential,operative
a|successful,prosperous,thriving,victorious,triumphant,flourishing,fruitful,accomplished
a|popular,well-liked,favoured,fashionable,in demand,admired,sought-after,widespread,common
a|public,communal,common,general,open,civic,shared,social,collective
a|private,personal,confidential,secret,individual,exclusive,intimate,secluded,own
a|secret,hidden,confidential,covert,private,concealed,undisclosed,clandestine,classified,veiled
a|general,broad,universal,overall,common,widespread,sweeping,blanket,generic
a|specific,particular,precise,definite,exact,explicit,distinct,detailed,individual
a|particular,specific,special,certain,distinct,individual,exact
a|detailed,thorough,comprehensive,full,elaborate,exhaustive,in-depth,itemized,meticulous
a|short,brief,concise,compact,succinct,little,terse,fleeting,abbreviated,curt
a|long,lengthy,extended,prolonged,protracted,extensive,drawn-out,elongated,tall
a|tall,high,lofty,towering,lanky,soaring,elevated,big
a|high,tall,elevated,lofty,towering,raised,soaring,steep,sky-high
a|low,short,small,shallow,squat,stunted,lowly,reduced,depressed
a|wide,broad,extensive,spacious,ample,vast,expansive,sweeping,roomy
a|narrow,thin,slim,tight,slender,cramped,confined,restricted,close
a|thin,slim,slender,lean,skinny,slight,narrow,fine,flimsy,sparse,lanky,gaunt
a|fat,plump,stout,overweight,heavy,chubby,portly,obese,rotund,bulky
a|heavy,weighty,hefty,bulky,massive,dense,substantial,burdensome,leaden,ponderous
a|light,weightless,airy,slight,thin,delicate,portable,feathery,flimsy
a|bright,brilliant,shining,vivid,luminous,radiant,dazzling,glowing,gleaming,sunny
a|dark,dim,gloomy,shadowy,murky,dusky,black,unlit,sombre,pitch-black
a|dull,boring,tedious,dreary,monotonous,uninteresting,humdrum,tiresome,flat,lifeless
a|boring,tedious,dull,monotonous,dreary,tiresome,uninteresting,repetitive,mind-numbing
a|interesting,fascinating,engaging,absorbing,intriguing,compelling,gripping,captivating,stimulating
a|exciting,thrilling,exhilarating,stimulating,gripping,stirring,electrifying,dramatic
a|surprising,astonishing,amazing,startling,unexpected,shocking,remarkable,staggering,astounding
a|amazing,astonishing,incredible,astounding,staggering,stunning,remarkable,wonderful,extraordinary
a|incredible,unbelievable,unthinkable,implausible,astonishing,amazing,far-fetched,improbable
a|wonderful,marvellous,fantastic,superb,terrific,fabulous,splendid,magnificent,delightful
a|great,grand,magnificent,impressive,eminent,remarkable,superb,splendid,majestic,illustrious
a|nice,pleasant,agreeable,enjoyable,delightful,lovely,charming,likeable,good,pleasing
a|pleasant,agreeable,pleasing,enjoyable,nice,delightful,congenial,genial,charming
a|awful,dreadful,terrible,appalling,ghastly,shocking,horrible,nasty,unpleasant
a|horrible,dreadful,awful,terrible,hideous,nasty,ghastly,revolting,vile,repulsive
a|nasty,unpleasant,horrible,disagreeable,vile,foul,offensive,repugnant,obnoxious
a|annoying,irritating,infuriating,bothersome,tiresome,vexing,troublesome,maddening
a|difficult,problematic,troublesome,awkward,thorny,knotty,perplexing,hard
a|simple,plain,basic,easy,uncomplicated,elementary,modest,clear,unadorned
a|complex,complicated,intricate,involved,elaborate,convoluted,tangled,sophisticated,multifaceted
a|confusing,puzzling,baffling,perplexing,bewildering,unclear,muddled,ambiguous,vague
a|vague,unclear,indistinct,hazy,imprecise,ambiguous,obscure,woolly,fuzzy,nebulous,loose
a|ambiguous,unclear,vague,equivocal,uncertain,cryptic,doubtful,obscure,open to question
a|strict,stern,severe,rigid,firm,harsh,exacting,rigorous,authoritarian,stringent
a|flexible,adaptable,pliable,supple,versatile,adjustable,elastic,accommodating,bendy
a|fair,just,impartial,equitable,objective,unbiased,even-handed,reasonable,honest
a|unfair,unjust,biased,partial,prejudiced,one-sided,inequitable,discriminatory
a|legal,lawful,legitimate,permitted,authorised,valid,licit,allowed,rightful
a|illegal,unlawful,illicit,criminal,banned,prohibited,forbidden,outlawed,against the law
a|free,unrestricted,liberated,independent,at liberty,complimentary,gratis,loose,costless
a|independent,self-reliant,autonomous,free,self-sufficient,self-governing,separate,sovereign
a|dependent,reliant,subordinate,contingent,conditional,needy
a|ancient,old,antique,archaic,prehistoric,primeval,age-old,bygone,early
a|final,last,ultimate,closing,concluding,end,terminal,definitive,eventual
a|first,initial,opening,earliest,original,introductory,primary,inaugural,premier
a|last,final,ultimate,closing,concluding,latest,rearmost,end
a|next,following,subsequent,succeeding,upcoming,adjacent,later,nearest
a|previous,earlier,prior,former,preceding,past,foregoing,antecedent
a|future,coming,forthcoming,approaching,later,prospective,expected,upcoming
a|current,present,existing,contemporary,ongoing,topical,prevailing,up-to-date
a|recent,latest,new,fresh,modern,late,current,up-to-date
a|permanent,lasting,enduring,constant,eternal,perpetual,fixed,stable,durable,everlasting
a|temporary,brief,short-term,interim,provisional,passing,fleeting,transient,momentary
a|constant,continuous,steady,unceasing,persistent,incessant,perpetual,regular,unchanging,stable
a|frequent,regular,repeated,common,numerous,recurrent,habitual,persistent
a|stable,steady,secure,firm,constant,sound,balanced,established,sturdy,fixed
a|steady,stable,constant,regular,even,consistent,fixed,unwavering,firm,reliable
a|reliable,dependable,trustworthy,faithful,consistent,steady,responsible,solid,sound
a|loyal,faithful,devoted,dedicated,true,steadfast,constant,staunch,committed
a|generous,giving,charitable,lavish,liberal,kind,unselfish,magnanimous,bountiful
a|selfish,self-centred,greedy,egocentric,mean,miserly,inconsiderate,grasping
a|greedy,avaricious,grasping,gluttonous,selfish,covetous,voracious,rapacious
a|mean,stingy,miserly,tight,cheap,tight-fisted,close,parsimonious,ungenerous
a|lucky,fortunate,blessed,favoured,charmed,jammy,providential
a|unlucky,unfortunate,hapless,ill-fated,cursed,jinxed,luckless
a|ill,sick,unwell,poorly,ailing,diseased,infirm,off colour,indisposed
a|healthy,well,fit,strong,hale,robust,sound,hearty,wholesome,in good shape
a|alive,living,live,breathing,animate,existing,vital,surviving
a|dead,deceased,lifeless,departed,late,extinct,gone,perished
a|empty,blank,bare,clear,void,vacant
a|certain,particular,specific,given,some
a|ideal,perfect,model,flawless,exemplary,supreme,optimal,textbook,best
a|perfect,flawless,ideal,faultless,immaculate,impeccable,spotless,exemplary,unblemished
a|best,finest,top,leading,greatest,foremost,optimum,supreme,prime
a|worst,lowest,poorest,most dreadful,least,rock-bottom
a|enough,adequate,plenty,sufficient,ample
a|wild,untamed,feral,savage,natural,uncultivated,unruly,fierce,rugged
a|natural,organic,native,innate,inborn,raw,unprocessed,inherent
a|artificial,synthetic,man-made,manufactured,fake,false,unnatural,mock
a|local,regional,neighbourhood,community,nearby,provincial,parochial,district
a|foreign,overseas,alien,exotic,external,imported,international,remote
a|international,global,worldwide,universal,cosmopolitan,intercontinental
a|whole,solid,sound,unbroken,intact,undamaged
a|official,formal,authorised,approved,legitimate,licensed,authoritative,sanctioned
a|formal,official,ceremonial,proper,conventional,stiff,solemn,correct
a|informal,casual,relaxed,unofficial,colloquial,easy-going,everyday,familiar
a|casual,informal,relaxed,offhand,nonchalant,carefree,easy-going,leisurely
a|physical,bodily,corporeal,material,tangible,fleshly,real
a|mental,intellectual,cerebral,psychological,rational,cognitive
a|academic,scholarly,educational,learned,intellectual,scholastic,theoretical
a|scientific,systematic,empirical,analytical,methodical,technical,experimental
a|logical,rational,reasonable,sensible,coherent,valid,sound,consistent
a|relevant,pertinent,applicable,germane,related,appropriate,apt,material
a|appropriate,suitable,proper,fitting,apt,right,fit,relevant,becoming,seemly
a|suitable,appropriate,fitting,proper,apt,right,fit,convenient,acceptable
a|adequate,sufficient,enough,satisfactory,acceptable,reasonable,passable,fair
a|acceptable,satisfactory,adequate,passable,reasonable,tolerable,fair,allowable
a|sufficient,enough,adequate,ample,satisfactory
a|particular,peculiar,special,characteristic,distinctive
a|typical,characteristic,representative,standard,usual,classic,average,normal,model
a|unique,singular,sole,exclusive,distinctive,one-off,unparalleled,matchless,individual
a|various,diverse,different,varied,assorted,miscellaneous,sundry,numerous,several
a|average,mean,typical,usual,ordinary,medium,standard,median,normal,moderate
a|moderate,reasonable,average,medium,modest,restrained,fair,mild,temperate
a|mild,gentle,soft,moderate,temperate,balmy,calm,tender,easy
a|total,complete,absolute,utter,entire,sheer,outright,thorough,overall,full
a|absolute,total,complete,utter,perfect,sheer,pure,unconditional,outright,supreme
a|obvious,glaring,blatant,flagrant,overt,conspicuous,patent,naked,clear
a|brief,short,quick,concise,fleeting,momentary,passing,succinct,cursory
a|abundant,plentiful,ample,copious,profuse,lavish,bountiful,rich,teeming
a|crucial,vital,critical,pivotal,essential,key,decisive,central,paramount
a|essential,vital,fundamental,indispensable,crucial,necessary,key,basic,intrinsic
a|significant,important,notable,meaningful,considerable,major,substantial,momentous,sizable
a|substantial,considerable,significant,sizeable,large,ample,solid,hefty,sturdy
a|considerable,significant,substantial,sizeable,notable,appreciable,fair,large,great
a|enormous,vast,huge,immense,gigantic,colossal,tremendous,massive
a|minor,small,slight,trivial,petty,unimportant,lesser,subordinate,insignificant
a|major,main,principal,chief,important,leading,key,significant,big,serious
a|possible,potential,likely,probable,prospective,imaginable,feasible,promising
a|potential,possible,prospective,likely,future,latent,budding,promising,aspiring
a|eager,keen,enthusiastic,avid,anxious,impatient,ardent,fervent,zealous,willing
a|enthusiastic,eager,keen,passionate,zealous,excited,avid,ardent,fervent,devoted
a|passionate,ardent,fervent,intense,zealous,fiery,heated,emotional,impassioned
a|curious,inquisitive,inquiring,interested,nosy,prying,questioning,intrigued
a|confident,assured,self-assured,certain,sure,positive,poised,self-confident,bold
a|determined,resolute,decided,firm,persistent,steadfast,tenacious,single-minded,committed
a|stubborn,obstinate,headstrong,pigheaded,dogged,inflexible,mulish,wilful,unyielding
a|patient,tolerant,forbearing,long-suffering,uncomplaining,calm,persevering,composed
a|anxious,worried,concerned,uneasy,apprehensive,troubled,fretful,tense,nervous
a|worried,anxious,concerned,troubled,uneasy,fretful,bothered,distressed,apprehensive
a|upset,distressed,troubled,hurt,disturbed,agitated,unhappy,dismayed,shaken,bothered
a|surprised,astonished,amazed,startled,astounded,shocked,taken aback,stunned,staggered
a|confused,puzzled,baffled,bewildered,perplexed,muddled,mystified,disoriented,flummoxed
a|embarrassed,ashamed,awkward,self-conscious,mortified,sheepish,humiliated,abashed
a|jealous,envious,covetous,resentful,green-eyed,grudging,begrudging
a|lonely,isolated,solitary,alone,forlorn,friendless,desolate,forsaken,secluded
a|alone,solitary,single,lone,isolated,solo,unaccompanied,by oneself,apart
a|sure,certain,positive,confident,definite,convinced,assured,secure
a|silly,foolish,ridiculous,absurd,daft,idiotic,frivolous,childish,senseless,inane
a|crazy,mad,insane,absurd,wild,foolish,bonkers,unhinged,deranged,lunatic
a|ridiculous,absurd,ludicrous,laughable,preposterous,silly,farcical,nonsensical,outrageous
a|strange,peculiar,curious,unusual,eerie,uncanny,weird,mysterious,odd
a|famous,renowned,celebrated,illustrious,prominent,legendary,noted,popular
a|rich,fertile,productive,fruitful,lush,abundant,plentiful
a|sharp,keen,pointed,acute,razor-sharp,cutting,edged,jagged,piercing
a|blunt,dull,rounded,unsharpened,obtuse,edgeless,worn
a|smooth,even,flat,level,sleek,glossy,silky,polished,flush,regular
a|rough,coarse,uneven,bumpy,rugged,rocky,jagged,irregular,bristly,scratchy
a|straight,direct,undeviating,unbent,level,linear,upright,true,uncurved
a|crooked,bent,twisted,curved,warped,askew,misshapen,winding,tilted
a|round,circular,spherical,rounded,curved,ring-shaped,globular
a|flat,level,even,smooth,horizontal,plane,flush,prone,low
a|steep,sheer,abrupt,precipitous,sharp,sudden,vertical,high,excessive
a|deep,profound,bottomless,yawning,cavernous,intense,heartfelt,extreme
a|shallow,superficial,surface,skin-deep,slight,trivial,flimsy,empty
a|thick,wide,broad,dense,stout,heavy,bulky,fat,viscous,solid
a|near,close,nearby,adjacent,neighbouring,handy,at hand,imminent,intimate
a|far,distant,remote,faraway,outlying,far-flung,removed,long
a|outer,outside,exterior,external,outermost,surface,outward,peripheral
a|inner,inside,interior,internal,innermost,central,intimate,private,hidden
a|upper,higher,top,superior,topmost,elevated,loftier
a|lower,bottom,inferior,under,beneath,reduced,junior,minor
a|entire,whole,complete,full,unbroken
v|say,state,declare,announce,remark,mention,utter,express,claim,assert,affirm,pronounce
v|tell,inform,notify,advise,instruct,report,reveal,relate,narrate,disclose
v|speak,talk,converse,chat,address,lecture,communicate,express,orate
v|talk,speak,chat,converse,discuss,gossip,chatter,natter,confer
v|ask,enquire,inquire,question,query,interrogate,quiz,request,demand,beg
v|answer,reply,respond,retort,rejoin,react,acknowledge,counter,return
v|explain,clarify,describe,illustrate,define,interpret,spell out,expound,justify,elucidate
v|describe,portray,depict,characterise,outline,report,illustrate,detail,recount,narrate
v|discuss,debate,talk over,consider,argue,examine,deliberate,confer,review
v|argue,debate,dispute,quarrel,contend,claim,maintain,assert,reason,bicker,hold
v|claim,assert,maintain,declare,allege,contend,insist,state,profess,hold,argue
v|suggest,propose,recommend,advise,imply,advocate,put forward,hint,submit,urge
v|recommend,advise,suggest,advocate,endorse,propose,urge,commend,counsel
v|mention,refer to,cite,note,touch on,allude to,name,remark,bring up,quote
v|cite,quote,mention,reference,name,refer to,adduce,allude to,instance
v|show,display,present,exhibit,reveal,demonstrate,indicate,illustrate,expose,point out,evidence
v|demonstrate,show,prove,illustrate,establish,exhibit,display,confirm,verify,manifest
v|prove,demonstrate,show,establish,verify,confirm,validate,substantiate,attest,evidence
v|indicate,show,suggest,signal,point to,denote,imply,mean,signify,reveal,designate
v|reveal,disclose,expose,uncover,divulge,unveil,show,betray,make known,release
v|hide,conceal,cover,mask,disguise,obscure,bury,veil,screen,cloak,stash
v|find,discover,locate,detect,uncover,spot,identify,unearth,come across,trace
v|discover,find,uncover,detect,learn,realise,unearth,identify,determine,reveal
v|look,glance,gaze,stare,peer,glimpse,peek,observe,watch,regard,view,peep
v|see,notice,spot,observe,perceive,glimpse,sight,witness,discern,detect,view,behold
v|watch,observe,view,monitor,look at,study,survey,regard,scrutinise,eye
v|examine,inspect,study,investigate,analyse,scrutinise,review,survey,explore,probe,check
v|study,examine,research,investigate,analyse,review,read,learn,explore,consider,survey
v|analyse,examine,study,evaluate,assess,investigate,interpret,break down,dissect,scrutinise
v|investigate,examine,explore,probe,research,look into,inquire into,inspect,study,scrutinise
v|research,investigate,study,explore,examine,survey,analyse,look into,delve into
v|consider,think about,contemplate,reflect on,weigh,ponder,regard,deliberate,examine,mull over,review
v|think,believe,suppose,reckon,imagine,consider,assume,suspect,judge,feel,deem,guess
v|believe,think,hold,trust,suppose,consider,maintain,credit,accept,presume,feel
v|know,understand,recognise,realise,comprehend,grasp,be aware of,appreciate,perceive
v|understand,grasp,comprehend,see,follow,realise,fathom,appreciate,take in,get
v|learn,study,master,discover,absorb,acquire,pick up,grasp,find out,memorise
v|teach,instruct,educate,train,tutor,coach,school,lecture,guide,inform,drill
v|remember,recall,recollect,reminisce,memorise,retain,think back,bear in mind
v|forget,overlook,neglect,omit,disregard,miss,lose sight of,leave out
v|decide,determine,resolve,choose,settle,conclude,opt,elect,rule,judge
v|choose,select,pick,opt for,decide on,elect,prefer,single out,adopt,nominate,take
v|select,choose,pick,elect,single out,prefer,opt for,appoint,nominate
v|want,desire,wish,crave,fancy,long for,need,covet,yearn for,hanker after
v|need,require,demand,want,call for,lack,necessitate,be short of
v|like,enjoy,love,fancy,appreciate,relish,prefer,adore,be fond of,admire
v|love,adore,cherish,treasure,worship,be devoted to,dote on,idolise,care for,like
v|hate,detest,loathe,despise,abhor,dislike,resent,abominate,can't stand
v|enjoy,like,relish,savour,love,appreciate,delight in,take pleasure in,revel in
v|fear,dread,be afraid of,be scared of,worry about,tremble at,panic about
v|worry,fret,be anxious,agonise,brood,be concerned,stress,trouble,bother
v|hope,wish,expect,anticipate,aspire,long,trust,look forward to,desire,dream
v|expect,anticipate,await,predict,assume,foresee,forecast,suppose,count on,look for
v|try,attempt,endeavour,strive,seek,aim,struggle,venture,undertake,essay
v|attempt,try,endeavour,undertake,seek,strive,venture,aim,make an effort
v|begin,start,commence,open,launch,initiate,originate,set about,embark on,kick off
v|start,begin,commence,launch,initiate,found,set off,open,trigger,set up,originate
v|end,finish,conclude,stop,terminate,complete,cease,close,wind up,halt,expire
v|finish,complete,end,conclude,accomplish,wrap up,close,fulfil,achieve,terminate,polish off
v|stop,halt,cease,quit,end,discontinue,pause,terminate,desist,prevent,block,arrest
v|continue,carry on,persist,proceed,keep on,last,go on,resume,maintain,persevere,sustain
v|keep,retain,hold,maintain,preserve,store,save,hang on to,conserve,possess,detain
v|hold,grasp,grip,clutch,carry,cling to,embrace,keep,clasp,support,have
v|take,grab,seize,grasp,pick up,capture,obtain,accept,receive,steal,carry,get
v|get,obtain,acquire,receive,gain,earn,secure,fetch,win,procure,attain,collect
v|obtain,get,acquire,gain,secure,procure,attain,receive,come by,earn
v|gain,obtain,win,earn,acquire,secure,achieve,attain,get,profit,collect,reap
v|give,provide,supply,offer,present,donate,grant,hand over,award,contribute,bestow,deliver
v|provide,supply,give,offer,furnish,equip,deliver,present,yield,afford,contribute
v|offer,propose,present,put forward,provide,tender,give,volunteer,submit,extend,bid
v|send,dispatch,post,mail,forward,transmit,ship,convey,issue,despatch,direct
v|bring,carry,fetch,deliver,take,bear,transport,convey,lead,produce
v|carry,bear,transport,convey,move,take,haul,lug,cart,transmit,support
v|move,shift,transfer,relocate,go,budge,proceed,travel,stir,advance,carry
v|go,travel,move,proceed,leave,depart,head,journey,pass,walk,advance
v|come,arrive,approach,reach,appear,turn up,show up,near,advance,get here
v|arrive,come,reach,get here,appear,turn up,show up,land,enter,pull in
v|leave,depart,go,exit,withdraw,quit,set off,abandon,retire,vacate,desert
v|return,come back,go back,revert,recur,reappear,give back,restore,replace,repay
v|enter,go in,come in,penetrate,access,join,invade,infiltrate,insert,record
v|run,sprint,race,dash,jog,rush,hurry,bolt,dart,gallop,speed,flee
v|walk,stroll,stride,amble,march,wander,pace,saunter,trek,hike,tread,step
v|jump,leap,spring,hop,bound,vault,skip,hurdle,pounce,bounce
v|fall,drop,tumble,collapse,plunge,descend,slip,decline,sink,topple,trip
v|rise,climb,ascend,go up,mount,soar,lift,increase,surge,arise,grow
v|climb,ascend,scale,mount,clamber,rise,go up,scramble,shin up
v|grow,expand,increase,develop,swell,enlarge,mature,flourish,sprout,thrive,rise,spread
v|increase,rise,grow,expand,raise,boost,enlarge,escalate,swell,extend,intensify,multiply,add to
v|decrease,reduce,lessen,decline,diminish,lower,cut,shrink,drop,fall,dwindle,curtail,wane
v|reduce,cut,lessen,decrease,lower,diminish,trim,shrink,minimise,curtail,slash,scale down
v|improve,enhance,better,upgrade,refine,develop,advance,progress,boost,amend,rally,polish
v|worsen,deteriorate,decline,degenerate,aggravate,exacerbate,weaken,slide,fail
v|change,alter,modify,adjust,transform,convert,vary,amend,revise,switch,shift,adapt,turn
v|alter,change,modify,amend,adjust,adapt,revise,vary,transform,rework,customise
v|transform,change,convert,alter,transmute,remodel,reshape,revolutionise,metamorphose,turn
v|adapt,adjust,modify,alter,convert,acclimatise,tailor,fit,accommodate,customise
v|make,create,produce,build,construct,form,manufacture,fashion,generate,craft,assemble,compose
v|create,make,produce,generate,form,invent,establish,originate,design,devise,fashion,build
v|build,construct,erect,assemble,make,fabricate,put up,form,raise,develop,establish
v|produce,make,create,generate,manufacture,yield,cause,turn out,bring about,give rise to
v|cause,produce,provoke,bring about,give rise to,generate,induce,trigger,lead to,result in,prompt
v|result,follow,ensue,arise,stem,emerge,develop,end,derive,spring
v|lead,guide,direct,head,conduct,steer,command,escort,pioneer,govern,manage,usher
v|guide,lead,direct,steer,conduct,escort,pilot,show,advise,counsel,instruct
v|manage,run,direct,control,administer,supervise,oversee,handle,organise,govern,cope,conduct
v|control,govern,manage,direct,regulate,command,rule,restrain,dominate,curb,operate
v|organise,arrange,plan,coordinate,order,sort,structure,systematise,set up,schedule,orchestrate
v|arrange,organise,order,sort,plan,set out,position,dispose,array,fix,prepare,schedule
v|plan,design,devise,organise,arrange,intend,map out,outline,scheme,propose,plot,aim
v|prepare,ready,get ready,arrange,make ready,organise,prime,equip,train,draft,set up
v|design,plan,devise,create,invent,draw up,sketch,outline,style,conceive,model,fashion
v|develop,evolve,grow,expand,advance,progress,mature,build,improve,cultivate,generate,elaborate
v|use,employ,utilise,apply,exercise,operate,exploit,wield,handle,consume,make use of,spend
v|apply,use,employ,utilise,exercise,implement,administer,put to use,practise,request,appeal
v|work,labour,toil,function,operate,perform,serve,slog,strive,run,act,go
v|operate,work,run,function,perform,handle,manage,use,control,go,act,drive
v|help,assist,aid,support,serve,benefit,cooperate,back,lend a hand,facilitate,abet,relieve
v|support,back,sustain,uphold,help,endorse,bolster,assist,defend,champion,underpin,prop
v|assist,help,aid,support,abet,serve,relieve,facilitate,cooperate,second
v|allow,permit,let,authorise,enable,sanction,approve,license,tolerate,grant,admit
v|permit,allow,authorise,let,license,sanction,enable,grant,warrant,empower,consent
v|prevent,stop,block,hinder,avert,prohibit,forestall,thwart,impede,deter,preclude,obstruct
v|ban,forbid,prohibit,bar,outlaw,veto,exclude,disallow,suppress,block,proscribe
v|stop,prevent,halt,block,hinder,curb,check,arrest,quell
v|avoid,evade,dodge,shun,escape,sidestep,elude,steer clear of,bypass,skirt,abstain from
v|escape,flee,run away,get away,break out,bolt,abscond,evade,avoid,elude,leak,decamp
v|protect,defend,guard,shield,safeguard,secure,preserve,shelter,cover,keep safe,conserve
v|defend,protect,guard,support,uphold,shield,champion,justify,vindicate,fortify,preserve
v|attack,assault,raid,strike,charge,storm,invade,ambush,criticise,set upon,bombard,pounce
v|fight,battle,combat,struggle,wrestle,brawl,clash,feud,scrap,contest,duel,quarrel
v|win,triumph,succeed,prevail,conquer,overcome,beat,secure,earn,achieve,gain,come first
v|beat,defeat,conquer,overcome,vanquish,best,outdo,trounce,thrash,hit,strike,surpass
v|lose,mislay,misplace,forfeit,drop,be defeated,fail,fall behind,waste,shed,miss
v|fail,flop,founder,miscarry,collapse,fall through,falter,go wrong,break down,neglect
v|succeed,triumph,prosper,thrive,flourish,achieve,accomplish,make it,win,prevail,follow
v|achieve,accomplish,attain,reach,realise,fulfil,earn,gain,complete,obtain,effect,succeed in
v|accomplish,achieve,complete,fulfil,carry out,realise,perform,execute,finish,attain
v|complete,finish,conclude,accomplish,end,fulfil,finalise,round off,wrap up,fill in
v|do,perform,carry out,execute,accomplish,achieve,undertake,conduct,complete,fulfil,act
v|perform,do,carry out,execute,carry through,accomplish,fulfil,act,play,present,function,render
v|carry out,perform,execute,conduct,implement,accomplish,fulfil,effect,discharge,undertake
v|implement,carry out,execute,enforce,apply,put into effect,fulfil,effect,realise,introduce
v|follow,pursue,trail,chase,track,shadow,tail,go after,succeed,obey,comply with,heed,come after
v|obey,comply with,follow,adhere to,observe,heed,submit to,conform to,respect,mind
v|ignore,disregard,overlook,neglect,snub,pass over,brush aside,turn a blind eye to,slight
v|accept,receive,take,agree to,welcome,admit,acknowledge,approve,adopt,embrace,swallow
v|reject,refuse,decline,turn down,spurn,rebuff,dismiss,deny,veto,discard,scorn,renounce
v|refuse,decline,reject,turn down,deny,withhold,say no,rebuff,spurn,balk
v|agree,concur,consent,assent,accept,comply,match,correspond,coincide,settle,see eye to eye
v|disagree,differ,dispute,object,argue,quarrel,clash,contradict,dissent,diverge,conflict
v|deny,contradict,refute,reject,dispute,disclaim,repudiate,rebut,negate,refuse,gainsay
v|admit,confess,acknowledge,concede,own up,grant,allow,reveal,accept,disclose,let in
v|confirm,verify,validate,corroborate,prove,establish,endorse,affirm,support,ratify,approve
v|check,verify,examine,inspect,test,confirm,monitor,review,investigate,control,curb,restrain
v|test,try,trial,examine,assess,check,evaluate,sample,experiment,probe,put to the test,screen
v|measure,gauge,assess,calculate,quantify,weigh,evaluate,size up,determine,survey,rate,estimate
v|estimate,calculate,reckon,gauge,judge,evaluate,guess,approximate,assess,value,rate
v|calculate,compute,work out,reckon,determine,count,figure,estimate,tally,assess
v|count,calculate,add up,tally,total,number,reckon,enumerate,tot up,matter
v|compare,contrast,juxtapose,weigh,liken,equate,match,balance,relate,measure
v|evaluate,assess,judge,appraise,rate,gauge,estimate,review,weigh up,analyse,size up
v|assess,evaluate,judge,appraise,gauge,rate,estimate,weigh,review,analyse,size up
v|judge,assess,evaluate,decide,conclude,rule,consider,appraise,deem,arbitrate,referee
v|criticise,condemn,censure,blame,attack,slam,find fault with,denounce,pan,disparage,fault
v|praise,compliment,commend,applaud,laud,acclaim,congratulate,extol,flatter,celebrate,applaud
v|blame,accuse,hold responsible,charge,condemn,criticise,fault,reproach,censure
v|accuse,charge,blame,indict,allege,denounce,impeach,arraign,incriminate
v|warn,caution,alert,advise,notify,forewarn,tip off,admonish,threaten,counsel
v|encourage,inspire,motivate,support,urge,stimulate,spur,hearten,cheer,promote,foster,rouse
v|persuade,convince,influence,induce,coax,sway,urge,win over,prevail upon,entice,talk into
v|convince,persuade,satisfy,assure,win over,sway,prove to,reassure
v|force,compel,make,oblige,coerce,pressure,drive,press,impose,push,constrain,necessitate
v|require,need,demand,call for,want,necessitate,involve,oblige,compel,order,take
v|demand,require,insist on,call for,claim,exact,order,request,need,necessitate,command
v|request,ask for,seek,beg,petition,solicit,appeal,entreat,call for,apply for,demand
v|order,command,instruct,direct,tell,require,bid,decree,dictate,arrange,sort,book
v|join,connect,link,unite,attach,combine,fasten,merge,couple,bond,fuse,add,enlist
v|connect,join,link,attach,couple,relate,associate,tie,fasten,bridge,unite
v|combine,mix,merge,blend,unite,join,integrate,fuse,amalgamate,pool,incorporate,bring together
v|mix,blend,combine,merge,stir,mingle,fuse,jumble,intermingle,toss,shuffle,socialise
v|separate,divide,split,part,detach,sever,break up,disconnect,isolate,segregate,disunite,sort
v|divide,split,separate,share,cut,partition,break up,carve up,segment,distribute,halve
v|share,divide,split,distribute,allot,apportion,partake,go halves,pool,give out
v|cut,slice,chop,carve,slash,trim,clip,sever,gash,snip,reduce,shorten,prune,hack
v|break,smash,shatter,crack,fracture,snap,split,burst,destroy,rupture,fail,violate,interrupt
v|destroy,demolish,ruin,wreck,devastate,wipe out,raze,annihilate,obliterate,shatter,smash
v|damage,harm,hurt,injure,impair,spoil,mar,wreck,ruin,deface,vandalise
v|harm,hurt,injure,damage,wound,abuse,impair,ill-treat,mistreat,disadvantage
v|hurt,harm,injure,wound,pain,damage,distress,upset,grieve,ache,sting,offend
v|repair,fix,mend,restore,patch,renovate,heal,remedy,service,overhaul,correct,rectify
v|fix,repair,mend,correct,attach,fasten,secure,settle,arrange,set,rectify,adjust,rig
v|clean,wash,cleanse,scrub,wipe,sweep,rinse,mop,purify,disinfect,launder,tidy
v|open,unlock,unfasten,uncover,undo,unwrap,unseal,expose,start,launch,unfold
v|close,shut,seal,fasten,lock,end,conclude,finish,bar,block,wind up,near
v|cover,wrap,coat,hide,conceal,protect,blanket,overlay,shield,spread,envelop,report
v|fill,stuff,pack,load,crowd,cram,occupy,supply,stock,block,plug,inflate,satisfy
v|empty,drain,clear,vacate,evacuate,unload,void,pour out,deplete,exhaust
v|remove,take away,withdraw,eliminate,delete,erase,extract,take off,detach,get rid of,abolish,strip
v|delete,erase,remove,cancel,cross out,strike out,wipe out,rub out,expunge,cut,obliterate
v|add,append,attach,include,tack on,sum,total,combine,insert,supplement,affix
v|include,contain,incorporate,comprise,cover,involve,embrace,encompass,add,take in,count
v|contain,hold,include,comprise,enclose,carry,embody,accommodate,restrain,control,curb
v|consist,comprise,be made up of,include,contain,be composed of,amount to
v|involve,include,entail,require,necessitate,implicate,embrace,take in,affect,engage,concern
v|affect,influence,impact,alter,change,concern,touch,move,sway,involve,apply to,disturb
v|influence,affect,sway,impact,shape,guide,control,direct,induce,persuade,pressure
v|depend,rely,count,hinge,lean,bank,trust,be contingent,turn,rest
v|rely,depend,count,bank,trust,lean,reckon,swear by
v|trust,believe in,rely on,have faith in,depend on,count on,confide in,expect,hope,credit
v|doubt,question,distrust,suspect,mistrust,be sceptical,query,challenge,hesitate
v|wait,stay,pause,hold on,linger,delay,hang on,remain,hesitate,hang about,expect,await
v|stay,remain,wait,linger,stop,reside,lodge,abide,continue,keep,tarry,dwell,visit
v|live,dwell,reside,exist,inhabit,stay,lodge,survive,abide,occupy,breathe
v|exist,be,live,survive,occur,prevail,remain,endure,subsist,breathe,abide
v|happen,occur,take place,arise,come about,befall,transpire,result,materialise,crop up,develop
v|occur,happen,take place,arise,come about,crop up,transpire,appear,exist,strike
v|appear,seem,look,emerge,show up,arrive,surface,materialise,turn up,come out,feature
v|seem,appear,look,sound,come across as,give the impression of,feel,strike one as
v|become,turn into,grow,get,develop into,change into,evolve into,come to be,suit,befit
v|remain,stay,continue,persist,last,endure,linger,stand,abide,survive,wait
v|sit,rest,perch,settle,squat,be seated,take a seat,pose,meet,convene
v|stand,rise,get up,be upright,stay,tolerate,endure,bear,put up with,withstand,place
v|lie,recline,rest,lounge,sprawl,repose,loll,be situated,be located,fib,deceive,falsify
v|sleep,doze,nap,slumber,rest,snooze,drowse,repose,kip,hibernate
v|eat,consume,devour,swallow,munch,gobble,dine,feast,chew,ingest,have,nibble
v|drink,sip,swallow,gulp,quaff,sup,imbibe,guzzle,swig,down,toast
v|buy,purchase,acquire,obtain,pay for,get,procure,invest in,shop for
v|sell,trade,market,retail,vend,peddle,auction,deal in,stock,hawk,flog
v|pay,reimburse,compensate,settle,remunerate,spend,refund,cough up,reward,give,yield
v|spend,pay out,expend,disburse,fork out,use up,squander,devote,pass,while away,lay out
v|waste,squander,fritter away,misuse,lavish,blow,dissipate,throw away,lose
v|save,store,keep,rescue,set aside,preserve,conserve,economise,put by,hoard,deliver,protect
v|earn,make,gain,receive,get,net,collect,gross,deserve,merit,win,obtain,bring in
v|cost,charge,fetch,come to,sell for,amount to,set back,price,require
v|write,compose,pen,draft,scribble,jot down,record,author,inscribe,note,put down,type
v|read,study,peruse,scan,browse,skim,look through,pore over,interpret,understand,decipher
v|draw,sketch,depict,trace,portray,outline,pull,drag,tug,haul,attract,engrave,tow
v|paint,colour,coat,decorate,depict,portray,daub,tint,cover,brush,stain,dye
v|sing,chant,warble,croon,hum,trill,carol,perform,serenade,chirp,yodel
v|play,act,perform,amuse oneself,frolic,romp,have fun,compete,participate,toy,gamble
v|laugh,chuckle,giggle,chortle,snigger,cackle,guffaw,titter,roar,crack up
v|cry,weep,sob,wail,bawl,blubber,whimper,snivel,shed tears,shout,yell,call,howl
v|shout,yell,cry,scream,bellow,roar,bawl,holler,call,exclaim,screech
v|whisper,murmur,mutter,mumble,breathe,hiss,sigh,rustle,undertone
v|smile,grin,beam,smirk,simper,look pleased,twinkle
v|touch,feel,handle,stroke,pat,brush,tap,contact,finger,graze,affect,reach
v|push,shove,press,thrust,propel,drive,nudge,force,prod,urge,jostle,advance
v|pull,drag,tug,haul,draw,tow,yank,heave,pluck,attract,strain,wrench
v|throw,toss,fling,hurl,cast,pitch,lob,chuck,sling,propel,launch,heave
v|catch,seize,grab,capture,trap,snatch,take,grasp,intercept,contract,hook,net,ensnare
v|hit,strike,punch,smack,slap,knock,thump,whack,bang,beat,clout,batter,swat,bash
v|kill,murder,slay,execute,assassinate,destroy,slaughter,eliminate,butcher,exterminate,end
v|die,perish,expire,pass away,decease,succumb,fade,decay,end,fail,croak
v|bury,inter,entomb,hide,conceal,sink,embed,cover,submerge,engulf
v|grab,seize,snatch,grasp,clutch,snap up,catch,take,pluck,nab,capture
v|release,free,liberate,let go,set free,discharge,unleash,emit,publish,issue,launch,loose
v|throw away,discard,dispose of,dump,scrap,jettison,get rid of,bin,ditch,reject
v|put,place,set,lay,position,deposit,locate,rest,stick,park,plant,situate
v|place,put,set,lay,position,locate,deposit,situate,station,rest,arrange,install
v|lift,raise,elevate,hoist,boost,pick up,heave,hold up,uplift,remove,cancel,steal,rise
v|lower,drop,reduce,let down,decrease,cut,dip,sink,drop,demote,lessen,depress,debase
v|attach,fasten,fix,join,connect,affix,stick,tie,secure,add,append,bind,couple
v|hang,suspend,dangle,drape,swing,droop,sag,hover,attach,hover,execute
v|turn,rotate,spin,revolve,twist,swivel,pivot,whirl,roll,swing,wheel,become,change,curve
v|shake,tremble,quiver,shiver,vibrate,rattle,jolt,wobble,quake,rock,shudder,wave
v|bend,curve,flex,twist,arch,bow,stoop,buckle,warp,crook,lean,fold
v|fold,crease,bend,pleat,double,tuck,wrap,overlap,collapse,go under
v|wrap,cover,envelop,swathe,enfold,pack,parcel,bundle,swaddle,shroud,sheathe,roll
v|stretch,extend,reach,expand,spread,lengthen,elongate,strain,pull,draw out,unfold,cover
v|fit,suit,match,belong,correspond,agree,install,adapt,tally,go,befit,be suitable
v|belong,fit,go,be part of,be owned by,pertain,be a member of,relate,have a place
v|suit,fit,become,flatter,match,satisfy,please,agree with,befit,adapt,go with
v|match,equal,rival,correspond,fit,go with,suit,agree,tally,parallel,pair,coordinate
v|differ,vary,contrast,diverge,disagree,deviate,depart,be unlike,conflict,dissent
v|vary,differ,change,fluctuate,alter,diversify,deviate,range,modify,diverge,oscillate
v|depend on,rely on,hinge on,rest on,turn on,count on,be contingent on
v|result in,lead to,cause,produce,bring about,give rise to,end in,generate,effect,entail
v|reflect,mirror,echo,show,indicate,express,display,think,ponder,consider,throw back
v|represent,stand for,symbolise,signify,depict,portray,denote,embody,act for,illustrate,express
v|mean,signify,denote,indicate,imply,suggest,represent,intend,convey,spell,stand for,purport
v|define,explain,specify,describe,determine,characterise,fix,establish,outline,spell out,delimit
v|identify,recognise,name,pinpoint,distinguish,single out,diagnose,spot,label,determine,establish
v|recognise,identify,know,acknowledge,appreciate,realise,accept,admit,notice,place,spot,salute
v|realise,understand,recognise,grasp,appreciate,comprehend,fulfil,achieve,accomplish,sell for,see
v|notice,see,observe,spot,detect,perceive,note,remark,heed,discern,mark,behold
v|note,observe,notice,record,register,write down,mention,remark,jot down,heed,mark,comment
v|record,note,register,document,log,enter,write down,chronicle,transcribe,tape,film,preserve
v|publish,issue,print,release,produce,announce,circulate,spread,broadcast,put out,disclose,bring out
v|announce,declare,proclaim,report,publicise,broadcast,reveal,make known,state,herald,advertise
v|advertise,promote,publicise,market,plug,push,announce,hype,advertise,tout,display
v|introduce,present,launch,bring in,establish,initiate,begin,pioneer,institute,insert,acquaint
v|present,give,offer,show,introduce,display,stage,exhibit,award,hand over,submit,host,render
v|submit,hand in,present,tender,offer,propose,yield,surrender,give in,put forward,suggest,comply
v|deliver,hand over,bring,convey,supply,give,distribute,transport,carry,present,pronounce,rescue
v|distribute,hand out,give out,deliver,share,disperse,circulate,spread,allocate,dispense,issue
v|collect,gather,assemble,accumulate,amass,pick up,compile,raise,bring together,muster,fetch,save
v|gather,collect,assemble,meet,accumulate,congregate,harvest,muster,pick,infer,summon,cluster
v|meet,encounter,come across,run into,assemble,gather,convene,greet,join,satisfy,face,fulfil
v|visit,call on,drop in on,see,stay with,stop by,tour,look in on,go to,haunt,attend
v|invite,ask,welcome,request,summon,bid,tempt,attract,call,court,provoke
v|welcome,greet,receive,embrace,hail,accept,salute,meet,be glad of,usher in
v|thank,show gratitude to,be grateful to,acknowledge,credit,appreciate,give thanks to
v|apologise,say sorry,express regret,ask forgiveness,make amends,repent,atone
v|forgive,pardon,excuse,absolve,overlook,let off,spare,exonerate,condone,acquit
v|punish,penalise,discipline,chastise,sentence,castigate,correct,fine,scold,chasten
v|reward,recompense,repay,compensate,pay,honour,award,remunerate,benefit,requite
v|offer,give,suggest,hold out,extend,present,propose,bid,provide
v|insist,demand,assert,maintain,persist,stress,declare,swear,claim,hold,press,urge
v|emphasise,stress,highlight,underline,underscore,accentuate,spotlight,draw attention to,point up,feature
v|highlight,emphasise,stress,spotlight,underline,underscore,accentuate,feature,mark,focus on
v|focus,concentrate,centre,fix,direct,aim,zero in,pinpoint,spotlight,converge,attend
v|concentrate,focus,centre,gather,condense,thicken,pay attention,apply oneself,cluster,converge
v|rely on,depend on,lean on,count on,bank on,trust,turn to
v|ensure,make sure,guarantee,secure,confirm,certify,warrant,assure,safeguard,see to,establish
v|achieve,reach,make,get,win,attain
v|aim,intend,aspire,plan,mean,strive,try,point,direct,target,propose,seek,train
v|intend,mean,plan,aim,propose,have in mind,contemplate,purpose,design,expect,destine
v|propose,suggest,put forward,offer,advance,recommend,intend,mean,present,nominate,submit
v|promote,advance,encourage,further,support,foster,boost,advertise,publicise,upgrade,endorse,push
v|establish,found,set up,create,institute,start,form,organise,prove,confirm,determine,install
v|maintain,keep,sustain,preserve,continue,retain,uphold,service,support,claim,assert,keep up
v|enhance,improve,increase,boost,enrich,heighten,strengthen,augment,intensify,upgrade,raise
v|extend,stretch,lengthen,expand,prolong,widen,enlarge,spread,reach,offer,carry on,elongate
v|expand,grow,enlarge,extend,increase,widen,swell,broaden,spread,develop,augment,amplify,elaborate
v|reduce,diminish,decline,shrink,subside,lessen,ease,dwindle,abate,wane,slacken
v|limit,restrict,confine,curb,restrain,cap,constrain,bound,ration,check,hamper,delimit
v|restrict,limit,confine,constrain,curb,regulate,restrain,hamper,cramp,impede,hem in,contain
v|prohibit,ban,forbid,bar,veto,outlaw,disallow,proscribe,rule out,prevent
v|obtain,receive,earn,take,secure
v|face,confront,meet,encounter,tackle,brave,oppose,look at,front,challenge,defy,deal with
v|solve,resolve,answer,crack,work out,fix,settle,unravel,decipher,clear up,figure out
v|resolve,settle,solve,decide,determine,sort out,fix,work out,conclude,clear up,agree
v|handle,deal with,manage,treat,cope with,tackle,control,touch,hold,manipulate,operate,wield
v|deal,handle,cope,manage,trade,distribute,tackle,sell,concern,treat,share out
v|cope,manage,get by,survive,make do,carry on,handle,deal,subsist,make out
v|struggle,strive,labour,fight,battle,grapple,toil,wrestle,work,endeavour,flounder
v|suffer,endure,experience,undergo,bear,tolerate,sustain,go through,ache,hurt,be afflicted
v|endure,bear,tolerate,suffer,withstand,stand,put up with,last,persist,survive,undergo
v|tolerate,endure,bear,stand,put up with,accept,allow,stomach,abide,permit,brook
v|enjoy,savour,like,appreciate,relish,have
v|experience,undergo,encounter,go through,feel,face,live through,meet,sense,suffer
v|feel,sense,touch,experience,perceive,handle,notice,think,believe,seem,grope,be aware of
v|sense,feel,detect,perceive,notice,realise,discern,suspect,be aware of,pick up
v|appreciate,value,prize,treasure,admire,respect,be grateful for,cherish,welcome,realise,understand
v|admire,respect,look up to,esteem,appreciate,approve of,revere,praise,applaud,venerate
v|respect,admire,esteem,honour,value,regard,revere,defer to,observe,heed,obey
v|value,prize,treasure,appreciate,cherish,esteem,rate,assess,respect,hold dear,estimate
v|worth,merit,deserve,warrant,be worthy of,justify,earn
v|compete,contend,vie,fight,rival,challenge,take part,participate,strive,struggle
v|participate,take part,join in,engage,partake,be involved,contribute,share,play a part
v|contribute,give,donate,add,provide,supply,subscribe,chip in,play a part,help,bestow
v|belong to,go with,be owned by,be part of
v|replace,substitute,supersede,succeed,supplant,take over from,change,swap,restore,put back,exchange
v|exchange,swap,trade,switch,barter,change,interchange,replace,substitute,convert
v|substitute,replace,swap,exchange,change,stand in for,deputise,switch,fill in,cover for
v|repeat,restate,reiterate,echo,recite,quote,duplicate,redo,say again,retell,replay
v|copy,duplicate,reproduce,imitate,replicate,mimic,clone,transcribe,photocopy,echo,ape
v|imitate,copy,mimic,mirror,emulate,echo,ape,parody,impersonate,simulate,follow
v|compare,liken,contrast
v|apply for,request,seek,petition,put in for,solicit,try for
v|refer,mention,allude,cite,direct,point,pass on,relate,concern,consult,apply,send
v|relate,connect,associate,link,tell,narrate,recount,report,concern,pertain,correlate,identify
v|connect,associate,link,relate
v|belong,pertain,relate,apply
v|conclude,finish,end,decide,deduce,infer,close,wind up,judge,settle,terminate,gather
v|infer,deduce,conclude,gather,derive,reason,understand,surmise,work out,guess
v|assume,suppose,presume,take for granted,suspect,believe,think,expect,adopt,take on,imagine
v|guess,estimate,suppose,speculate,reckon,surmise,hypothesise,conjecture,imagine,predict,fancy
v|predict,forecast,foresee,foretell,anticipate,expect,prophesy,project,divine,augur,call
v|imagine,picture,envisage,visualise,suppose,conceive,dream up,fancy,think,suspect,assume,fantasise
v|dream,imagine,fantasise,visualise,hope,aspire,wish,hallucinate,reverie,envisage
v|wonder,ponder,speculate,question,ask oneself,think,marvel,be amazed,muse,doubt,query
v|ponder,consider,think about,reflect on,contemplate,muse,deliberate,mull over,wonder,meditate
v|reflect on,contemplate,consider,ponder,think over,review,muse over
v|refer to,mention,cite,quote,allude to,speak of,touch on
v|rest,relax,repose,pause,unwind,take it easy,idle,sleep,lie down,lean,depend,remainder
v|relax,rest,unwind,calm down,loosen up,chill,ease,laze,lighten up,soothe,slacken
v|hurry,rush,hasten,speed,dash,scurry,race,run,hustle,get a move on,fly,expedite
v|delay,postpone,defer,put off,hold up,hinder,detain,stall,hold back,suspend,shelve,linger
v|postpone,delay,defer,put off,shelve,suspend,adjourn,hold over,put back,reschedule
v|cancel,call off,abandon,scrap,annul,revoke,repeal,delete,drop,abort,withdraw,void
v|try out,test,sample,evaluate,experiment with,put to the test,trial
v|experiment,test,try,trial,research,investigate,examine,explore,sample,pilot
v|explore,investigate,examine,research,probe,look into,study,survey,scout,travel,inspect,delve into
v|search,look for,seek,hunt,scour,comb,rummage,forage,quest,explore,probe,investigate,frisk
v|seek,look for,search for,pursue,hunt,try,aim,attempt,strive,request,ask for,want
v|pursue,chase,follow,hunt,track,trail,seek,strive for,engage in,persist in,continue,go after
v|chase,pursue,follow,hunt,track,run after,trail,hound,drive away,rush
v|escape from,flee,run away from,get away from
v|ride,drive,travel,cycle,journey,sit,go,motor,steer
v|drive,steer,motor,propel,push,ride,operate,compel,force,urge,herd,power
v|fly,soar,glide,flit,hover,wing,take off,pilot,flutter,hurry,flee,zoom
v|swim,bathe,float,paddle,dip,wade,dive,splash,glide,flood
v|sink,descend,submerge,drop,fall,go under,plunge,decline,founder,slump,drown,dip
v|float,drift,bob,glide,hover,hang,waft,swim,levitate,sail,suspend
v|flow,run,stream,pour,drift,gush,course,spill,glide,ripple,cascade,circulate,issue
v|pour,flow,stream,gush,spill,tip,decant,course,cascade,teem,flood,rain,serve
v|burn,blaze,flame,ignite,scorch,char,singe,incinerate,smoulder,glow,flare,sear,torch,kindle
v|shine,gleam,glow,glitter,sparkle,beam,twinkle,radiate,flash,glisten,glint,excel,polish
v|grow up,mature,develop,come of age,age,advance
v|raise,lift,elevate,hoist,boost,increase,rear,bring up,erect,collect,breed,muster,heighten
v|bring up,rear,raise,nurture,educate,foster,care for,breed,mention,broach,introduce
v|care,look after,tend,mind,nurse,protect,watch over,worry,attend,provide for,mind,bother
v|look after,care for,tend,mind,nurse,take care of,attend to,protect,watch over,guard
v|take care of,look after,care for,tend,nurse,deal with,handle,see to
v|serve,attend,wait on,help,assist,work for,supply,provide,function,act,present,dish out
v|work out,calculate,solve,figure out,devise,plan,exercise,train,compute,resolve,succeed
v|figure out,work out,understand,solve,calculate,fathom,decipher,crack,comprehend,suss out
v|sort,arrange,organise,classify,categorise,group,order,rank,grade,separate,class,type
v|classify,categorise,sort,group,grade,rank,class,organise,catalogue,pigeonhole,label
v|label,tag,mark,name,brand,identify,call,classify,title,ticket,designate,stamp
v|name,call,label,term,dub,title,designate,christen,identify,nominate,mention,specify,style
v|call,name,phone,ring,summon,shout,cry,label,term,dub,telephone,visit,hail,describe
v|contact,reach,get in touch with,approach,call,phone,write to,communicate with,touch,notify
v|communicate,convey,express,share,contact,correspond,exchange,transmit,connect,impart,relay
v|discuss,talk about,debate,consider,examine,go over,deliberate,review,argue,thrash out
v|conduct,carry out,run,direct,organise,lead,perform,manage,guide,transmit,behave,escort
v|perform,play,stage,present,act,appear,enact,render
v|operate on,treat,cut,perform surgery on
v|treat,handle,deal with,cure,heal,medicate,care for,regard,consider,entertain,pay for,doctor,tend
v|cure,heal,remedy,treat,restore,fix,make well,rectify,relieve,correct,preserve
v|heal,cure,recover,mend,restore,remedy,get better,regenerate,treat,soothe
v|recover,get better,heal,recuperate,regain,retrieve,rally,improve,revive,restore,get back,reclaim
v|survive,live,endure,last,outlive,remain,persist,pull through,continue,exist,withstand
v|vanish,disappear,fade,evaporate,dissolve,melt away,go,cease,pass,die out,dematerialise
v|disappear,vanish,fade,go,evaporate,melt away,cease,pass,die out,recede
v|grow smaller,shrink,contract,decrease,dwindle,diminish,wither,shrivel,narrow
v|shrink,contract,diminish,dwindle,shrivel,lessen,reduce,decrease,retreat,recoil,flinch,wither
v|swell,expand,bulge,balloon,enlarge,distend,inflate,bloat,increase,surge,billow,puff up
v|wear,have on,dress in,sport,put on,don,be clothed in,erode,fray,wear away,last,carry
v|dress,clothe,attire,robe,garb,deck,array,costume,bandage,decorate,fit out,season
v|decorate,adorn,ornament,embellish,beautify,garnish,trim,furnish,paint,paper,honour,deck
v|prepare food,cook,make,fix,rustle up
v|cook,prepare,make,bake,fry,roast,boil,grill,heat,stew,simmer,concoct,steam
v|bake,cook,roast,heat,fire,harden,parch
v|pick,choose,select,pluck,gather,harvest,collect,cull,decide on,nominate,opt for,single out
v|pick up,lift,collect,gather,raise,take,fetch,learn,acquire,catch,arrest,resume,improve
v|get up,rise,stand,arise,wake,stir,mount,climb
v|get on,board,mount,climb,progress,fare,manage,agree,cope,be friendly
v|get off,alight,dismount,disembark,leave,escape,descend,depart,exit
v|set,place,put,lay,arrange,fix,establish,decide,adjust,harden,solidify,determine,assign,sink
v|set up,establish,found,arrange,organise,start,create,build,erect,install,prepare,institute
v|set off,leave,depart,start out,begin,trigger,detonate,ignite,activate,provoke,start
v|take part,participate,join in,engage,play a part,share,get involved
v|take off,remove,doff,leave the ground,lift off,depart,mimic,deduct,discard,strip off,become popular
v|take place,happen,occur,go on,come about,arise,transpire,befall,materialise
v|take on,hire,employ,assume,accept,undertake,engage,face,tackle,adopt,acquire,recruit
v|give up,quit,abandon,stop,cease,surrender,renounce,resign,yield,forgo,relinquish,desist,submit
v|give in,surrender,yield,submit,concede,capitulate,cave in,relent,back down
v|put off,postpone,delay,defer,discourage,deter,repel,distract,dissuade,disconcert
v|put up with,tolerate,endure,bear,stand,stomach,abide,take,accept
v|look for,seek,search for,hunt for,try to find,quest for,forage for
v|look at,regard,view,watch,examine,consider,observe,study,survey,eye,gaze at,inspect
v|look into,investigate,examine,research,explore,check out,probe,study,inquire into
v|look up,consult,research,search for,find,improve,visit,get better
v|find out,discover,learn,detect,establish,uncover,realise,ascertain,determine,get to know
v|carry on,continue,go on,keep on,persist,proceed,maintain,persevere,behave,misbehave
v|go on,continue,carry on,proceed,last,happen,occur,keep on,persist,chatter,endure
v|go up,rise,climb,ascend,increase,mount,soar,escalate,surge,grow
v|go down,fall,decrease,drop,decline,descend,sink,set,diminish,lessen,collapse
v|go away,leave,depart,disappear,vanish,fade,retreat,withdraw
v|come up,arise,occur,appear,emerge,approach,crop up,rise,surface,materialise,sprout
v|come back,return,revisit,reappear,recur,retort,reply,regain
v|come across,find,meet,encounter,discover,stumble on,chance upon,seem,appear,run into
v|come out,emerge,appear,be published,become known,debut,bloom,turn out,surface,result
v|turn up,arrive,appear,come,show up,materialise,find,discover,increase,attend,surface
v|turn out,prove,result,happen,emerge,end up,produce,make,attend,evict,expel,come
v|break down,collapse,fail,stop working,analyse,decompose,weep,fall apart,malfunction,conk out
v|break up,split,disband,separate,disperse,dissolve,part,divorce,end,fragment,scatter
v|work on,tackle,concentrate on,develop,improve,polish,pursue,refine
v|carry through,complete,execute,accomplish,finish,see through
v|make up,invent,compose,fabricate,form,constitute,create,concoct,compensate,reconcile,comprise
v|make sure,ensure,check,verify,confirm,guarantee,see to it,make certain,secure
v|make out,discern,distinguish,perceive,see,understand,fathom,decipher,pretend,claim,write out
v|make use of,use,employ,utilise,exploit,apply,harness,take advantage of
v|pay attention,heed,listen,mind,attend,take note,notice,concentrate,observe
v|pay for,buy,fund,finance,settle,foot the bill for,reimburse,stand,cover
v|pay back,repay,reimburse,refund,return,compensate,get even with,retaliate,settle
v|keep up,maintain,sustain,continue,preserve,retain,uphold,persist,stay level
v|keep out,exclude,bar,shut out,ban,forbid,prohibit,debar,stay away,stop
v|keep going,continue,persist,carry on,persevere,press on,go on,last,soldier on
v|hold on,wait,grip,cling,persevere,keep,persist,stay,hang on,endure,remain
v|hold back,restrain,suppress,check,withhold,hinder,hesitate,curb,retain,control,contain,stifle
v|hold up,delay,support,rob,sustain,prop,stop,hinder,postpone,survive,endure,raise
v|let,allow,permit,authorise,enable,rent,lease,sanction,grant,suffer,hire out
v|let go,release,free,drop,relinquish,set free,loose,unclasp,liberate,discharge,dismiss
v|let down,disappoint,fail,betray,dishearten,lower,deflate,leave in the lurch
v|rule,govern,reign,control,lead,decide,judge,administer,command,dominate,determine,prevail
v|govern,rule,control,run,reign,administer,manage,lead,regulate,direct,command,steer,oversee
v|vote,elect,ballot,choose,poll,opt,decide,cast a vote,back,select
v|elect,choose,vote for,select,pick,appoint,opt for,decide,prefer,return,nominate,make
v|appoint,assign,name,select,nominate,hire,choose,delegate,install,designate,commission,elect,employ
v|employ,hire,engage,take on,recruit,use,utilise,appoint,retain,apply,exploit,occupy
v|hire,employ,engage,rent,lease,take on,recruit,appoint,charter,sign up,book
v|fire,dismiss,sack,discharge,let go,lay off,axe,terminate,shoot,launch,ignite,inspire,kindle
v|dismiss,fire,sack,discharge,release,reject,disband,drop,send away,let go,disregard,axe
v|resign,quit,step down,leave,stand down,retire,give notice,abdicate,hand in notice,relinquish
v|retire,withdraw,retreat,stop working,go to bed,leave,depart,quit,pull out,step down
v|retreat,withdraw,back off,fall back,retire,recede,pull back,flee,draw back,shrink,recoil
v|advance,proceed,progress,move forward,go on,push on,improve,develop,promote,further,lend,propose
v|progress,advance,proceed,develop,improve,move on,get on,gain ground,come along,evolve,headway
v|surpass,exceed,outdo,beat,top,eclipse,outshine,transcend,better,outstrip,excel,pass
v|exceed,surpass,go beyond,outdo,top,beat,overstep,outstrip,transcend,pass
v|overcome,conquer,beat,defeat,surmount,master,triumph over,overpower,get over,prevail,vanquish,subdue
v|struggle with,wrestle with,grapple with,battle with,contend with
v|rely upon,depend on,count on
v|damage,spoil,impair,hurt,mar
v|sacrifice,give up,forgo,surrender,offer up,forfeit,renounce,yield,let go
v|stimulate,encourage,spur,provoke,arouse,excite,inspire,prompt,rouse,trigger,incite,foster
v|inspire,encourage,motivate,stimulate,influence,fire,spur,move,prompt,breathe,instil,galvanise
v|motivate,inspire,drive,stimulate,prompt,encourage,spur,move,propel,activate,trigger
v|confuse,baffle,bewilder,puzzle,perplex,mystify,muddle,bemuse,disorient,fluster,mix up,jumble
v|surprise,astonish,amaze,startle,astound,stun,shock,stagger,take aback,flabbergast,catch off guard
v|shock,appal,horrify,stun,outrage,scandalise,disgust,jolt,traumatise,startle,offend,dismay
v|annoy,irritate,bother,irk,vex,anger,pester,exasperate,aggravate,nettle,provoke,rile,bug
v|please,delight,satisfy,amuse,gratify,content,charm,cheer,suit,thrill,humour,enchant
v|entertain,amuse,divert,delight,please,charm,host,receive,consider,cheer,engage,regale
v|amuse,entertain,divert,delight,cheer,tickle,occupy,charm,please,absorb,interest
v|interest,intrigue,fascinate,attract,engage,appeal to,absorb,capture,concern,involve,curiosity
v|attract,draw,pull,lure,entice,charm,tempt,appeal to,interest,fascinate,allure,magnetise
v|disturb,bother,interrupt,upset,trouble,unsettle,distress,agitate,disrupt,annoy,alarm,worry
v|disrupt,interrupt,disturb,upset,interfere with,unsettle,break up,disorganise,derail,hinder,obstruct
v|interfere,meddle,intrude,butt in,intervene,hinder,obstruct,tamper,pry,disrupt,conflict
v|interrupt,cut in,break in,intrude,disturb,butt in,suspend,cut short,stop,discontinue,heckle,barge in
v|threaten,menace,intimidate,endanger,jeopardise,warn,bully,imperil,terrorise,loom,browbeat
v|force,push,press,drive,make
v|encourage,urge,support,promote,foster,cheer on,boost,embolden
v|cheat,deceive,swindle,trick,defraud,con,fool,dupe,betray,mislead,rip off,fleece,hoodwink
v|deceive,mislead,fool,trick,cheat,dupe,delude,hoodwink,con,betray,hoax,take in,lead on
v|steal,rob,take,thieve,pinch,nick,swipe,pilfer,loot,burgle,purloin,filch,lift
v|lie down,recline,rest,stretch out,lounge,repose,sprawl,prostrate
v|obtain a copy,get,find,secure
v|apply oneself,work hard,concentrate,try hard,persevere,devote oneself,strive,buckle down
n|house,home,dwelling,residence,abode,property,cottage,villa,habitat,lodging,building
n|home,house,residence,dwelling,abode,base,domicile,household,family,haven
n|building,structure,edifice,construction,premises,erection,property,block,pile,complex
n|room,chamber,space,area,hall,compartment,capacity,scope,margin,office,leeway
n|city,town,metropolis,municipality,urban area,conurbation,borough,capital
n|town,city,borough,municipality,village,settlement,township,community
n|village,hamlet,settlement,community,township,small town,outpost
n|country,nation,state,land,realm,kingdom,republic,territory,homeland,countryside,rural area
n|nation,country,state,land,republic,realm,people,society,community,kingdom
n|world,earth,globe,planet,universe,cosmos,creation,humankind,society,sphere
n|place,spot,location,site,position,area,locale,setting,point,venue,seat,region,situation
n|area,region,zone,district,section,sector,territory,locality,part,field,space,quarter,extent
n|region,area,district,zone,territory,province,part,sector,quarter,domain,locality,belt
n|space,room,gap,area,expanse,capacity,void,extent,gap,interval,blank,universe
n|road,street,way,avenue,lane,highway,route,path,track,drive,boulevard,roadway
n|path,track,trail,way,route,lane,footpath,course,pathway,walkway,passage,alley
n|way,route,path,road,course,method,manner,means,approach,technique,mode,style,direction
n|method,way,approach,technique,procedure,process,system,manner,means,practice,strategy,mode
n|process,procedure,method,operation,system,course,practice,development,sequence,technique,action
n|system,method,scheme,network,structure,arrangement,organisation,framework,process,approach,order
n|approach,method,way,technique,strategy,attitude,style,manner,means,procedure,path
n|technique,method,skill,approach,way,procedure,system,knack,craft,manner,art
n|strategy,plan,scheme,tactics,policy,approach,programme,blueprint,design,method,game plan
n|plan,scheme,strategy,design,programme,project,proposal,idea,intention,blueprint,map,layout,aim
n|idea,thought,notion,concept,theory,plan,suggestion,impression,belief,view,opinion,brainwave,inkling
n|thought,idea,notion,thinking,reflection,consideration,opinion,belief,view,concept,contemplation
n|concept,idea,notion,theory,thought,conception,abstraction,principle,hypothesis,image,view
n|theory,hypothesis,thesis,idea,concept,principle,philosophy,conjecture,model,speculation,supposition
n|opinion,view,belief,judgement,thought,idea,viewpoint,stance,attitude,position,perspective,feeling,verdict
n|view,opinion,belief,attitude,outlook,perspective,standpoint,scene,sight,vista,panorama,landscape,vision,position
n|attitude,outlook,position,stance,approach,view,mindset,disposition,posture,perspective,manner
n|perspective,view,outlook,angle,standpoint,viewpoint,attitude,context,slant,position,vantage point
n|problem,difficulty,trouble,issue,complication,obstacle,snag,predicament,dilemma,setback,hitch,puzzle
n|issue,matter,topic,subject,problem,question,concern,point,affair,theme,edition,question,release
n|difficulty,problem,trouble,hardship,challenge,obstacle,complication,struggle,snag,predicament,hurdle
n|challenge,test,trial,difficulty,obstacle,dare,demand,hurdle,confrontation,provocation
n|obstacle,barrier,hurdle,hindrance,obstruction,impediment,snag,block,difficulty,hitch,bar,stumbling block
n|answer,solution,reply,response,resolution,key,explanation,remedy,retort,rejoinder,fix
n|solution,answer,remedy,key,resolution,fix,explanation,way out,result,cure,formula
n|question,query,enquiry,inquiry,problem,issue,matter,doubt,point,interrogation,uncertainty,topic
n|reason,cause,motive,grounds,basis,explanation,rationale,justification,purpose,excuse,logic,sense,ground
n|cause,reason,source,origin,root,basis,grounds,agent,producer,motive,principle,movement,ideal
n|effect,result,outcome,consequence,impact,influence,upshot,repercussion,end,fruit,impression
n|result,outcome,consequence,effect,upshot,product,end,conclusion,issue,verdict,score,answer
n|consequence,result,outcome,effect,repercussion,upshot,aftermath,sequel,importance,significance
n|impact,effect,influence,consequence,collision,impression,repercussion,bearing,force,shock,clash
n|influence,effect,impact,power,sway,control,authority,leverage,pull,clout,weight,bearing
n|purpose,aim,goal,objective,intention,target,point,function,reason,plan,design,use,resolve
n|aim,goal,objective,purpose,target,intention,ambition,end,aspiration,object,plan,design
n|goal,aim,objective,target,purpose,ambition,end,intention,aspiration,destination,object
n|objective,aim,goal,target,purpose,object,intention,ambition,end,mission,aspiration
n|result,fruit,product
n|chance,opportunity,possibility,likelihood,probability,opening,prospect,risk,fortune,luck,occasion,gamble
n|opportunity,chance,opening,occasion,possibility,break,time,prospect,scope,window,moment
n|risk,danger,hazard,threat,peril,chance,gamble,jeopardy,possibility,exposure,speculation,venture
n|danger,peril,hazard,risk,threat,menace,jeopardy,pitfall,insecurity,trouble
n|threat,danger,menace,risk,hazard,warning,intimidation,peril,omen,hint,sign
n|help,aid,assistance,support,backing,service,benefit,relief,cooperation,advice,guidance,hand
n|support,backing,help,aid,assistance,encouragement,advocacy,prop,foundation,base,brace,sustenance
n|aid,help,assistance,support,relief,benefit,backing,donation,service,cooperation
n|advice,guidance,counsel,recommendation,suggestion,tip,direction,help,opinion,information,instruction
n|information,data,facts,details,knowledge,news,intelligence,evidence,material,particulars,input,advice
n|data,information,facts,figures,statistics,details,evidence,input,material,findings,numbers
n|fact,truth,reality,detail,certainty,actuality,datum,point,particular,circumstance,event,statistic
n|truth,fact,reality,accuracy,honesty,verity,actuality,veracity,candour,sincerity,validity,gospel
n|lie,falsehood,untruth,fib,fabrication,fiction,deception,myth,invention,whopper,story,tale
n|evidence,proof,testimony,confirmation,grounds,data,support,documentation,verification,indication,sign,witness
n|proof,evidence,confirmation,verification,testimony,demonstration,documentation,corroboration,validation,test
n|example,instance,case,illustration,sample,specimen,model,demonstration,precedent,pattern,exemplar,ideal
n|instance,example,case,occasion,illustration,occurrence,sample,situation,event,time,precedent
n|case,instance,example,situation,occasion,circumstance,suit,lawsuit,action,patient,box,container,argument
n|sample,example,specimen,model,instance,taste,test,swatch,cross-section,representative,pattern,snippet
n|model,example,pattern,design,prototype,template,type,standard,ideal,version,replica,paragon,mould,mock-up
n|pattern,design,model,template,motif,arrangement,sequence,order,system,plan,decoration,norm,trend
n|type,kind,sort,variety,category,class,genre,style,form,species,brand,breed,group,nature
n|kind,type,sort,variety,class,category,form,brand,style,genre,breed,nature,ilk,gentle,caring
n|sort,kind,type,variety,class,category,genre,order,brand,species,group,family,nature
n|group,set,collection,batch,band,class,category,cluster,company,gang,party,team,bunch,body,gathering
n|set,group,collection,series,batch,kit,assortment,range,array,class,sequence,suite,circle,clique
n|collection,group,set,assortment,accumulation,array,cluster,hoard,pile,anthology,selection,gathering,stock
n|team,group,crew,squad,side,company,band,party,gang,unit,line-up,staff
n|party,celebration,gathering,function,get-together,social,do,reception,bash,faction,group,side,band
n|company,firm,business,corporation,enterprise,organisation,establishment,concern,outfit,partnership,guests,companionship
n|business,company,firm,trade,commerce,industry,enterprise,occupation,work,affair,concern,profession,operation
n|organisation,institution,company,association,body,group,society,outfit,establishment,agency,club,order,structure
n|job,work,occupation,employment,career,profession,position,post,trade,task,duty,role,situation,chore
n|work,job,labour,employment,effort,toil,task,occupation,duty,activity,industry,profession,craft,creation
n|task,job,duty,chore,assignment,mission,undertaking,work,project,exercise,responsibility,errand
n|duty,responsibility,obligation,task,role,job,function,charge,commitment,burden,tax,tariff,allegiance
n|role,part,function,position,job,task,duty,capacity,character,place,responsibility,post
n|part,piece,section,portion,segment,fragment,component,element,share,bit,division,unit,role,member
n|piece,part,bit,portion,fragment,section,slice,segment,chunk,component,item,scrap,morsel,article
n|portion,part,share,piece,section,helping,quota,serving,segment,division,fraction,allotment
n|section,part,division,segment,portion,piece,sector,department,unit,district,chapter,passage,branch
n|element,component,part,factor,ingredient,constituent,feature,aspect,unit,piece,detail,member
n|factor,element,component,feature,consideration,aspect,influence,cause,ingredient,point,circumstance,determinant
n|feature,characteristic,aspect,attribute,quality,trait,property,facet,element,highlight,mark,hallmark,article
n|aspect,feature,facet,element,side,characteristic,angle,part,point,detail,appearance,look
n|characteristic,feature,trait,quality,attribute,property,mark,peculiarity,hallmark,idiosyncrasy,typical,distinctive
n|quality,characteristic,attribute,feature,property,trait,standard,grade,class,calibre,excellence,merit,nature,worth
n|property,possession,belongings,assets,estate,land,holdings,building,attribute,quality,characteristic,trait,wealth
n|thing,object,item,article,entity,device,something,matter,affair,fact,detail,gadget,material,business
n|object,item,thing,article,entity,body,aim,goal,purpose,target,target,thing,intent,goal
n|item,article,thing,object,piece,element,entry,point,detail,unit,product,feature,matter,topic
n|product,goods,item,merchandise,commodity,result,output,creation,artefact,produce,outcome,fruit,wares
n|goods,products,merchandise,wares,commodities,stock,items,belongings,property,freight
n|tool,instrument,implement,device,utensil,gadget,appliance,machine,apparatus,means,aid,contraption
n|machine,device,appliance,engine,apparatus,mechanism,tool,motor,contraption,instrument,robot
n|device,gadget,tool,instrument,appliance,machine,apparatus,mechanism,gizmo,contraption,scheme,ploy
n|material,substance,matter,stuff,fabric,cloth,medium,textile,data,information,content,raw
n|substance,material,matter,stuff,essence,body,fabric,gist,meaning,solidity,wealth,drug,pith
n|money,cash,funds,currency,capital,finance,wealth,means,coin,notes,resources,riches,dough
n|cost,price,charge,fee,expense,rate,amount,payment,outlay,expenditure,toll,value,sacrifice,damage
n|price,cost,charge,fee,rate,amount,value,fare,tariff,expense,toll,figure,penalty
n|fee,charge,payment,price,cost,rate,toll,fare,tariff,bill,remuneration,subscription,commission
n|profit,gain,earnings,revenue,income,return,proceeds,yield,benefit,surplus,advantage,margin,winnings
n|loss,defeat,failure,deficit,damage,waste,forfeit,deprivation,disappearance,bereavement,death,cost
n|income,earnings,salary,pay,wages,revenue,takings,profits,means,receipts,proceeds,remuneration
n|salary,pay,wage,earnings,income,remuneration,stipend,fee,payment,compensation,screw
n|debt,obligation,liability,arrears,dues,bill,loan,overdraft,duty,commitment,deficit,owing
n|benefit,advantage,gain,profit,help,asset,good,boon,blessing,perk,value,aid,welfare,reward
n|advantage,benefit,asset,plus,edge,merit,strength,gain,lead,superiority,profit,favour,upper hand
n|disadvantage,drawback,handicap,downside,weakness,minus,snag,shortcoming,limitation,liability,flaw
n|problem,trouble,worry,bother,nuisance,burden,hassle
n|mistake,error,blunder,fault,slip,oversight,flaw,inaccuracy,gaffe,miscalculation,lapse,bloomer,howler
n|error,mistake,fault,flaw,blunder,inaccuracy,slip,lapse,oversight,fallacy,defect,bug
n|fault,defect,flaw,failing,weakness,error,mistake,imperfection,blame,responsibility,shortcoming,glitch
n|flaw,defect,fault,imperfection,blemish,weakness,failing,shortcoming,weak spot,error,drawback,bug
n|failure,defeat,collapse,flop,breakdown,fiasco,disaster,shortcoming,loser,washout,bankruptcy,omission
n|success,triumph,victory,achievement,accomplishment,prosperity,hit,winner,fame,fortune,sensation,breakthrough
n|victory,win,triumph,success,conquest,mastery,superiority,achievement,walkover,coup
n|achievement,accomplishment,attainment,feat,success,triumph,deed,realisation,exploit,effort,fulfilment
n|progress,advance,development,headway,improvement,growth,evolution,advancement,breakthrough,step forward,movement
n|improvement,progress,development,advance,enhancement,recovery,upgrade,betterment,gain,reform,rally,boost
n|development,growth,progress,evolution,advance,expansion,maturity,improvement,event,occurrence,change,building
n|growth,increase,expansion,rise,development,enlargement,progress,spread,boom,swelling,tumour,stretch
n|change,alteration,modification,shift,variation,transformation,difference,revision,switch,adjustment,coins,conversion
n|difference,distinction,contrast,disparity,variation,gap,change,dissimilarity,divergence,discrepancy,variety,balance
n|similarity,resemblance,likeness,parallel,correspondence,analogy,affinity,closeness,kinship,sameness
n|relationship,connection,link,association,bond,tie,relation,correlation,affiliation,rapport,kinship,affair
n|connection,link,relationship,association,bond,tie,relation,contact,attachment,junction,context,network
n|link,connection,tie,bond,association,relationship,attachment,relation,relation,junction,hyperlink,loop
n|contact,touch,connection,communication,association,acquaintance,link,reach,junction
n|communication,contact,message,correspondence,exchange,dialogue,transmission,connection,link,talk,announcement
n|message,note,communication,letter,memo,word,notice,report,bulletin,missive,point,theme,moral,errand
n|conversation,talk,chat,discussion,dialogue,exchange,discourse,conference,gossip,tete-a-tete,parley
n|discussion,debate,conversation,talk,dialogue,argument,deliberation,exchange,consultation,conference,seminar
n|argument,dispute,quarrel,debate,disagreement,row,reasoning,case,claim,controversy,clash,squabble,plea,tiff
n|disagreement,dispute,argument,quarrel,conflict,difference,clash,discord,dissent,row,controversy,wrangle
n|conflict,fight,struggle,clash,dispute,battle,war,disagreement,combat,strife,contest,friction,discord
n|war,conflict,fighting,warfare,combat,battle,hostilities,struggle,campaign,crusade,strife,bloodshed
n|peace,calm,harmony,tranquillity,quiet,serenity,truce,stillness,concord,ceasefire,order,armistice
n|battle,fight,combat,clash,conflict,struggle,war,skirmish,engagement,campaign,crusade,action,encounter
n|agreement,deal,accord,pact,contract,treaty,arrangement,understanding,settlement,bargain,harmony,consent,concord
n|contract,agreement,deal,pact,commitment,bond,arrangement,treaty,covenant,understanding,bargain,settlement
n|rule,regulation,law,principle,guideline,order,standard,precept,decree,norm,custom,direction,reign,control
n|law,rule,regulation,statute,act,code,decree,legislation,order,principle,commandment,bill,measure,edict
n|policy,strategy,plan,approach,procedure,guideline,code,line,programme,protocol,principles,practice
n|principle,rule,law,standard,belief,ethic,doctrine,theory,tenet,fundamental,morality,truth,idea,creed
n|standard,level,norm,criterion,benchmark,yardstick,measure,model,grade,quality,requirement,flag,ideal,principle
n|example,ideal
n|rule,guideline
n|right,entitlement,privilege,claim,authority,freedom,licence,prerogative,due,liberty,power,title
n|freedom,liberty,independence,autonomy,release,licence,latitude,scope,discretion,openness,immunity
n|power,strength,force,might,energy,authority,control,influence,capacity,ability,command,sway,potency,vigour
n|force,power,strength,energy,might,violence,pressure,coercion,impact,intensity,drive,momentum,compulsion
n|energy,power,vigour,vitality,strength,drive,stamina,zest,fuel,electricity,dynamism,spirit,pep,life
n|strength,power,might,force,vigour,toughness,muscle,robustness,intensity,stamina,asset,strong point,potency
n|weakness,frailty,feebleness,flaw,fault,failing,defect,shortcoming,vulnerability,fondness,soft spot,debility
n|ability,skill,capacity,talent,capability,competence,aptitude,power,gift,knack,expertise,faculty,flair
n|skill,ability,talent,expertise,proficiency,competence,craft,technique,knack,art,aptitude,mastery,flair,finesse
n|talent,gift,ability,aptitude,flair,skill,genius,knack,faculty,bent,capacity,endowment,forte
n|knowledge,understanding,awareness,learning,information,expertise,education,wisdom,grasp,familiarity,scholarship,know-how
n|education,schooling,teaching,learning,instruction,training,tuition,study,tutoring,coaching,scholarship,cultivation
n|training,instruction,education,coaching,preparation,schooling,tuition,teaching,practice,discipline,drill,exercise
n|lesson,class,session,lecture,tutorial,period,seminar,exercise,moral,warning,example,teaching,assignment
n|school,academy,institution,college,seminary,institute,faculty,department,group,movement,shoal
n|student,pupil,learner,scholar,schoolchild,undergraduate,trainee,apprentice,disciple,graduate,novice
n|teacher,instructor,tutor,educator,lecturer,professor,schoolmaster,coach,mentor,guide,trainer,master,don
n|expert,specialist,authority,professional,master,pundit,connoisseur,maestro,adept,pro,ace,veteran,buff
n|leader,head,chief,director,boss,principal,commander,captain,ruler,manager,guide,chair,pioneer,front-runner
n|boss,manager,head,chief,leader,supervisor,director,employer,foreman,overseer,gaffer,principal
n|manager,director,head,boss,supervisor,administrator,executive,controller,organiser,overseer,governor
n|worker,employee,labourer,hand,staff member,operative,workman,artisan,member of staff,breadwinner
n|friend,companion,mate,pal,ally,partner,buddy,chum,associate,comrade,confidant,acquaintance,supporter
n|enemy,foe,opponent,adversary,rival,antagonist,nemesis,competitor,opposition,hostile,combatant
n|partner,associate,colleague,ally,collaborator,companion,spouse,mate,accomplice,teammate,consort,peer
n|colleague,co-worker,associate,workmate,partner,collaborator,teammate,fellow worker,ally,peer,comrade
n|member,part,element,limb,associate,participant,fellow,constituent,subscriber,component,representative
n|person,individual,human,being,human being,soul,character,someone,mortal,body,personage,creature
n|people,persons,folk,individuals,humans,public,citizens,population,society,nation,race,community,men and women
n|human,person,individual,mortal,being,man,woman,creature,soul,body
n|man,male,gentleman,guy,fellow,chap,bloke,boy,individual,person,human,adult male,husband
n|woman,female,lady,girl,dame,adult female,lass,wife,she
n|child,kid,youngster,boy,girl,youth,minor,toddler,infant,juvenile,offspring,descendant,nipper
n|baby,infant,newborn,babe,toddler,tot,bairn,suckling,little one
n|adult,grown-up,mature person,man,woman,grownup,elder
n|family,relatives,relations,household,kin,folk,clan,tribe,dynasty,lineage,parentage,ancestry,kindred
n|parent,mother,father,guardian,carer,progenitor,sire,dam,begetter,mum,dad
n|ancestor,forefather,forebear,predecessor,forerunner,precursor,antecedent,progenitor,primogenitor
n|society,community,civilisation,culture,people,public,population,world,association,club,fellowship,company,nation
n|community,society,neighbourhood,population,public,district,commonwealth,locality,colony,group,populace
n|culture,civilisation,society,customs,way of life,heritage,traditions,lifestyle,refinement,breeding,education,arts
n|tradition,custom,convention,practice,habit,ritual,heritage,folklore,institution,usage,observance,lore
n|custom,tradition,habit,practice,convention,routine,ritual,usage,fashion,way,trade,patronage,rule
n|habit,practice,custom,routine,tendency,pattern,addiction,wont,convention,mannerism,way,vice,quirk
n|routine,procedure,practice,habit,schedule,pattern,regime,custom,drill,programme,act,usual,regular
n|history,past,record,chronicle,account,story,annals,background,background,archives,heritage,antiquity,narrative
n|past,history,yesterday,former times,antiquity,bygone days,olden days,earlier times,background
n|future,tomorrow,time to come,prospect,outlook,destiny,hereafter,expectation,fate,forthcoming
n|time,period,moment,age,era,occasion,span,duration,stretch,interval,date,epoch,hour,spell,phase,tempo
n|period,time,era,age,epoch,phase,stage,span,spell,term,duration,interval,season,stretch,generation
n|moment,instant,second,minute,flash,jiffy,trice,tick,point,time,occasion,importance,juncture
n|instant,moment,second,flash,twinkling,trice,minute,tick,occasion,point,time
n|age,era,period,epoch,time,generation,years,aeon,date,stage,eon,lifetime,day,maturity
n|day,daytime,date,time,period,age,era,heyday,epoch,twenty-four hours,daylight
n|year,twelve months,annum,twelvemonth,calendar year,season,term
n|beginning,start,opening,origin,outset,onset,commencement,dawn,birth,launch,source,inception,genesis,introduction,germ
n|start,beginning,opening,outset,onset,commencement,launch,origin,dawn,kickoff,inception,departure,advantage,lead
n|end,finish,conclusion,close,ending,termination,finale,completion,expiry,culmination,aim,goal,stop,limit,tip,extremity
n|conclusion,end,finish,close,ending,termination,finale,completion,result,verdict,inference,deduction,outcome,decision,summary
n|origin,source,beginning,root,cause,start,birth,derivation,basis,heritage,ancestry,descent,provenance,genesis,dawn
n|source,origin,root,beginning,cause,spring,fountain,supplier,informant,authority,derivation,reference,wellspring
n|basis,foundation,base,ground,footing,principle,core,premise,starting point,support,rationale,groundwork,essence
n|foundation,basis,base,footing,groundwork,underpinning,bedrock,establishment,institution,endowment,founding,charity
n|base,bottom,foot,foundation,bed,support,stand,basis,headquarters,camp,centre,station,core
n|centre,middle,heart,core,hub,midpoint,focus,nucleus,focal point,pivot,crux,kernel,midst,bullseye
n|middle,centre,midpoint,heart,core,midst,median,halfway,mean,thick,waist,midsection
n|edge,border,rim,margin,boundary,brink,verge,side,lip,limit,fringe,periphery,perimeter,advantage,sharpness
n|border,boundary,edge,frontier,margin,limit,rim,perimeter,fringe,borderline,brink,verge,line
n|side,edge,flank,face,surface,wing,aspect,facet,part,team,party,camp,view,faction,margin,border
n|top,peak,summit,crest,apex,pinnacle,head,crown,tip,height,zenith,cap,lid,upper surface,best,culmination
n|bottom,base,foot,floor,foundation,underside,depths,lowest part,seat,rear,backside,nadir,bed
n|front,fore,face,facade,head,lead,forefront,vanguard,beginning,exterior,fore part,van
n|back,rear,reverse,end,stern,tail,backside,hind part,other side,spine
n|surface,exterior,face,top,outside,covering,veneer,finish,facade,skin,appearance,plane,area
n|shape,form,figure,outline,structure,contour,silhouette,profile,configuration,pattern,condition,guise,mould,cut
n|form,shape,figure,structure,outline,kind,type,design,format,document,arrangement,class,fitness,condition,mould
n|size,dimensions,magnitude,extent,proportions,measurements,volume,bulk,scale,area,mass,amount,largeness,range
n|amount,quantity,number,total,sum,volume,extent,mass,measure,bulk,supply,aggregate,expanse,lot
n|number,figure,digit,numeral,integer,total,amount,quantity,count,sum,issue,crowd,statistic,character
n|quantity,amount,number,volume,total,sum,measure,extent,proportion,mass,size,bulk,lot
n|total,sum,whole,aggregate,entirety,tally,amount,totality,full amount,result,grand total,overall
n|sum,total,amount,aggregate,tally,whole,quantity,calculation,problem,number,entirety
n|average,mean,median,norm,par,standard,typical,midpoint,usual,ordinary,rule,run
n|majority,most,bulk,mass,greater part,lion's share,preponderance,generality,adulthood,age of consent,plurality
n|minority,few,smaller part,less than half,fewer,small group,lesser number,childhood,youth
n|lot,great deal,plenty,heap,mass,abundance,quantity,load,pile,deal,stack,plethora,batch,fate,destiny,plot
n|bit,piece,part,fragment,scrap,morsel,crumb,speck,fraction,shred,jot,little,slice,snippet,drop,tad
n|pair,couple,duo,two,twosome,brace,twins,match,set of two,twin
n|couple,pair,duo,two,twosome,brace,partners,few,handful,twosome
n|series,sequence,succession,chain,string,run,set,order,progression,course,row,cycle,line,range
n|sequence,order,series,succession,progression,chain,course,arrangement,pattern,cycle,flow,string
n|list,catalogue,register,roll,inventory,record,index,schedule,table,directory,tally,roster,enumeration,file
n|table,chart,list,schedule,index,diagram,graph,grid,matrix,catalogue,bench,counter,board,desk
n|chart,graph,diagram,table,map,plan,plot,graphic,blueprint,tabulation,figure,illustration
n|picture,image,photograph,photo,illustration,drawing,painting,portrait,sketch,print,snapshot,depiction,representation,view,scene
n|image,picture,photo,likeness,representation,reflection,icon,figure,illustration,portrait,idea,impression,reputation
n|photo,picture,photograph,snap,snapshot,shot,print,image,still,portrait
n|painting,picture,canvas,portrait,landscape,artwork,work of art,oil,watercolour,mural,illustration
n|book,volume,publication,text,work,tome,title,novel,paperback,hardback,manual,textbook,publication,publication
n|story,tale,narrative,account,history,anecdote,yarn,saga,fiction,legend,report,plot,account,fable,article
n|article,piece,essay,item,story,feature,report,paper,column,composition,object,thing,commodity,clause,section
n|essay,composition,article,paper,piece,dissertation,thesis,treatise,assignment,study,discourse,tract,attempt
n|paper,newspaper,document,essay,article,thesis,report,journal,sheet,stationery,certificate,dissertation,study,daily,examination
n|report,account,description,statement,record,article,study,paper,review,write-up,bulletin,summary,communique,rumour,bang,explosion
n|document,paper,record,file,certificate,report,form,text,deed,manuscript,instrument,archive,evidence
n|text,wording,words,content,script,body,passage,book,textbook,transcript,version,words,original,extract,reading
n|word,term,expression,name,term,utterance,remark,comment,statement,promise,news,message,command,order,password,vocable
n|sentence,line,statement,phrase,clause,verdict,judgement,punishment,ruling,penalty,decision,condemnation
n|name,title,label,designation,term,identity,reputation,tag,handle,epithet,moniker,fame,celebrity,character
n|title,name,heading,caption,label,designation,headline,rank,position,championship,crown,right,ownership,claim
n|summary,synopsis,outline,abstract,digest,overview,review,precis,recap,resume,rundown,gist,condensation
n|introduction,preface,foreword,opening,prologue,preamble,beginning,launch,presentation,debut,initiation,lead-in,start
n|description,account,portrayal,depiction,explanation,report,sketch,characterisation,representation,details,narrative,kind,sort
n|explanation,account,reason,justification,description,clarification,interpretation,definition,answer,excuse,key,rationale
n|definition,explanation,meaning,description,interpretation,clarity,sharpness,focus,statement,specification
n|meaning,sense,significance,definition,import,purport,implication,gist,drift,message,point,interpretation,substance,intention
n|sense,feeling,awareness,perception,intuition,wisdom,judgement,meaning,logic,reason,faculty,sensation,point,purpose,impression
n|feeling,emotion,sensation,sense,sentiment,impression,belief,idea,mood,passion,instinct,hunch,suspicion,atmosphere
n|emotion,feeling,sentiment,passion,sensation,reaction,response,excitement,warmth,fervour
n|mood,humour,temper,spirit,disposition,state of mind,atmosphere,frame of mind,vein,tone,morale,whim,sulk
n|love,affection,fondness,devotion,passion,adoration,attachment,warmth,tenderness,liking,desire,infatuation,care,romance
n|hate,hatred,loathing,hostility,detestation,animosity,enmity,aversion,abhorrence,antipathy,revulsion
n|fear,terror,fright,dread,alarm,panic,horror,anxiety,apprehension,worry,phobia,trepidation,unease
n|anger,rage,fury,wrath,temper,annoyance,irritation,indignation,outrage,ire,resentment,spleen
n|joy,happiness,delight,pleasure,glee,bliss,elation,gladness,rapture,jubilation,cheer,ecstasy,merriment
n|sorrow,sadness,grief,misery,unhappiness,woe,heartache,regret,anguish,distress,melancholy,mourning,pain
n|pain,ache,hurt,soreness,suffering,agony,discomfort,pang,twinge,distress,anguish,torment,irritation,nuisance
n|pleasure,enjoyment,delight,joy,happiness,satisfaction,amusement,gratification,fun,contentment,entertainment,bliss
n|fun,enjoyment,amusement,entertainment,pleasure,play,recreation,merriment,sport,diversion,joking,good time,jollity
n|hope,wish,desire,aspiration,dream,expectation,ambition,optimism,faith,promise,prospect,belief,longing
n|dream,vision,fantasy,hope,ambition,aspiration,goal,reverie,delusion,wish,nightmare,imagination,daydream
n|ambition,aspiration,goal,aim,dream,desire,drive,objective,enterprise,determination,target,hunger,purpose
n|courage,bravery,nerve,pluck,valour,daring,fearlessness,heroism,guts,boldness,spirit,grit,backbone
n|patience,tolerance,endurance,forbearance,calmness,restraint,composure,perseverance,resignation,fortitude,persistence
n|attention,notice,heed,concentration,focus,awareness,regard,mind,thought,consideration,care,observation,note,interest
n|interest,curiosity,attention,concern,fascination,appeal,importance,significance,hobby,pastime,stake,share,involvement,intrigue,charge
n|care,attention,caution,concern,thought,heed,protection,custody,charge,supervision,worry,burden,upkeep,keeping
n|effort,attempt,endeavour,exertion,labour,work,energy,toil,struggle,try,strain,trouble,achievement,industry
n|attempt,try,effort,endeavour,bid,go,shot,stab,crack,venture,experiment,trial
n|trouble,difficulty,problem,worry,bother,distress,hardship,disturbance,unrest,nuisance,inconvenience,pain,strife,effort
n|worry,anxiety,concern,fear,unease,care,trouble,distress,apprehension,burden,problem,misgiving,stress
n|stress,strain,pressure,tension,anxiety,worry,emphasis,importance,weight,accent,force,hassle,demand,burden
n|pressure,force,stress,strain,weight,tension,compulsion,urgency,burden,influence,demands,compression,coercion
n|tension,strain,stress,pressure,anxiety,unease,nervousness,suspense,tightness,hostility,stiffness
n|surprise,shock,astonishment,amazement,bombshell,revelation,jolt,wonder,start,stupefaction,bolt from the blue
n|shock,surprise,blow,upset,jolt,bombshell,trauma,fright,disturbance,impact,collision,scare,start
n|mystery,puzzle,enigma,secret,riddle,conundrum,question,unknown,poser,problem,curiosity
n|secret,mystery,confidence,private matter,key,recipe,formula,enigma,hidden
n|surprise,astonishment,wonder
n|accident,mishap,crash,collision,misfortune,disaster,calamity,blunder,chance,fluke,coincidence,smash,pile-up
n|disaster,catastrophe,calamity,tragedy,misfortune,fiasco,failure,blow,cataclysm,mishap,debacle,ruin
n|crisis,emergency,disaster,catastrophe,predicament,turning point,critical point,plight,dilemma,trouble,climax
n|event,occurrence,incident,happening,episode,affair,occasion,experience,function,competition,fixture,circumstance,development
n|incident,event,occurrence,episode,happening,occasion,affair,disturbance,scene,clash,commotion,brush
n|situation,circumstances,state of affairs,position,condition,case,scenario,plight,predicament,location,job,place,site
n|condition,state,situation,shape,form,circumstance,fitness,order,health,requirement,proviso,stipulation,term,ailment
n|state,condition,situation,shape,form,mood,nation,country,province,territory,government,declare,say,affirm
n|status,position,standing,rank,level,prestige,state,condition,grade,class,situation,stature,importance
n|position,place,location,site,spot,situation,rank,status,job,post,posture,stance,view,opinion,standpoint,arrangement
n|point,tip,spot,end,detail,item,aim,purpose,place,position,moment,stage,argument,idea,sense,meaning,dot
n|detail,particular,point,fact,feature,element,item,aspect,respect,specific,fine point,minutiae,trifle,factor
n|point of view,viewpoint,standpoint,perspective,angle,opinion,stance,outlook,position
n|stage,phase,step,point,period,platform,level,juncture,division,leg,lap,theatre,podium,arena
n|step,stage,phase,move,action,measure,stride,pace,footstep,level,rung,tread,gesture,procedure,degree
n|level,degree,standard,grade,stage,rank,height,layer,position,storey,plane,class,echelon,tier,status
n|degree,extent,level,amount,measure,grade,stage,rank,qualification,diploma,intensity,magnitude,proportion,class
n|range,scope,extent,span,area,variety,spread,series,selection,sweep,reach,gamut,compass,field,assortment,limits
n|scope,range,extent,reach,sweep,coverage,span,area,room,opportunity,latitude,breadth,field,capacity
n|limit,boundary,restriction,maximum,edge,ceiling,end,cap,border,extent,curb,frontier,threshold,constraint
n|boundary,border,limit,edge,margin,frontier,perimeter,bounds,fringe,dividing line,confines,barrier
n|restriction,limit,restraint,curb,control,constraint,limitation,rule,regulation,check,condition,handicap
n|choice,option,selection,alternative,pick,preference,decision,election,variety,range,vote,say,range
n|option,choice,alternative,possibility,selection,preference,course of action,opportunity,pick,solution
n|alternative,option,choice,other,substitute,different,another,replacement,possibility,second,backup
n|decision,choice,resolution,verdict,conclusion,judgement,ruling,determination,finding,settlement,outcome,decree
n|opinion,judgement,verdict,assessment,estimate,evaluation,appraisal,ruling
n|test,examination,trial,exam,check,assessment,experiment,quiz,evaluation,analysis,probe,audition,investigation,proof
n|exam,examination,test,paper,assessment,quiz,viva,oral,final,trial,midterm
n|experiment,test,trial,investigation,study,research,procedure,examination,demonstration,tryout,pilot
n|study,research,investigation,analysis,survey,review,examination,inquiry,paper,report,work,learning,reading,den,office,education
n|research,investigation,study,inquiry,analysis,examination,experimentation,exploration,fact-finding,scrutiny,probe
n|survey,poll,study,review,analysis,examination,questionnaire,overview,inspection,census,appraisal,canvass,inquiry
n|analysis,examination,study,investigation,assessment,evaluation,interpretation,breakdown,scrutiny,review,dissection,test
n|review,assessment,evaluation,critique,appraisal,examination,report,survey,analysis,notice,study,revision,summary,criticism
n|criticism,censure,disapproval,condemnation,attack,fault-finding,critique,review,analysis,comment,flak,judgement,disparagement
n|comment,remark,statement,observation,note,opinion,criticism,commentary,mention,reaction,utterance,annotation,explanation
n|remark,comment,observation,statement,mention,utterance,note,reflection,declaration,word,opinion,assertion
n|observation,remark,comment,notice,watching,study,examination,view,monitoring,surveillance,finding,scrutiny,statement,note
n|finding,discovery,result,conclusion,outcome,verdict,decision,judgement,ruling,data,evidence,observation,upshot
n|discovery,find,finding,breakthrough,invention,detection,revelation,disclosure,innovation,uncovering,exploration,unearthing
n|invention,creation,innovation,discovery,device,design,gadget,brainchild,fabrication,fiction,lie,originality,development
n|innovation,novelty,invention,change,creativity,originality,modernisation,advance,departure,breakthrough,new idea
n|technology,science,engineering,machinery,hardware,gadgetry,know-how,mechanics,apparatus,devices,systems,electronics
n|science,knowledge,study,discipline,research,branch of knowledge,learning,expertise,skill,art,technique
n|nature,environment,world,creation,wildlife,countryside,universe,character,essence,quality,kind,type,disposition,temperament,sort
n|environment,surroundings,setting,habitat,milieu,conditions,atmosphere,background,territory,situation,context,nature
n|climate,weather,temperature,atmosphere,conditions,mood,trend,ambience,feeling,environment,spirit,tendency,clime
n|weather,climate,conditions,temperature,forecast,atmospheric conditions,elements,outlook,meteorology
n|sea,ocean,waters,deep,main,briny,waves,tide,expanse,mass,swell
n|water,liquid,fluid,rain,moisture,drink,aqua,sea,lake,river,tears
n|river,stream,waterway,brook,creek,tributary,watercourse,flow,channel,torrent,rivulet,beck
n|lake,pond,pool,loch,lagoon,mere,reservoir,tarn,water,inland sea
n|mountain,peak,hill,mount,summit,height,alp,elevation,ridge,range,fell,mound,pile
n|hill,mound,rise,slope,hillock,knoll,fell,height,elevation,incline,ridge,down,mountain,bank
n|forest,woods,woodland,jungle,trees,plantation,timberland,thicket,copse,grove,wilderness,bush
n|tree,plant,shrub,sapling,conifer,bush,timber,hardwood,evergreen,oak,pine,log
n|plant,flower,vegetation,herb,bush,shrub,weed,seedling,factory,works,mill,machinery,equipment,place,set
n|flower,bloom,blossom,bud,floret,petal,posy,plant,pick,best,cream,flourish
n|animal,creature,beast,brute,mammal,pet,organism,living thing,being,wildlife,critter,fauna,savage
n|dog,hound,pooch,canine,puppy,mutt,pup,cur,mongrel,pet
n|cat,feline,kitten,tabby,puss,moggy,tom,pet,tomcat,kitty
n|bird,fowl,songbird,chick,feathered friend,nestling,flyer,raptor
n|fish,seafood,catch,aquatic creature,swimmer,angle,trawl
n|food,nourishment,fare,meal,sustenance,provisions,nutrition,diet,cuisine,refreshments,groceries,eats,grub,feed
n|meal,repast,feast,dinner,lunch,breakfast,supper,spread,snack,banquet,tea,bite,course
n|drink,beverage,liquid,refreshment,cocktail,pint,tipple,glass,sip,swallow,draught,alcohol,booze,round
n|bread,loaf,roll,toast,bun,baguette,bap,staple
n|car,automobile,vehicle,motor,motor car,saloon,coupe,sedan,banger,wheels,carriage,jalopy
n|vehicle,car,truck,van,bus,transport,conveyance,carrier,means,medium,machine,automobile,motor
n|ship,boat,vessel,craft,liner,steamer,sailing boat,tanker,ferry,yacht,freighter
n|boat,vessel,craft,ship,dinghy,yacht,ferry,barge,launch,cruiser,canoe,skiff
n|journey,trip,voyage,tour,expedition,excursion,travel,passage,trek,outing,odyssey,drive,flight,ride,crossing
n|trip,journey,excursion,outing,tour,expedition,jaunt,voyage,visit,drive,stumble,fall,slip,errand
n|travel,journey,tourism,touring,voyages,wandering,globetrotting,movement,transit,trek,going
n|visit,call,stay,stopover,trip,social call,tour,appointment,stop,sojourn,viewing,visitation
n|visitor,guest,caller,tourist,company,traveller,visitant,sightseer,holidaymaker,passenger
n|guest,visitor,caller,company,diner,patron,boarder,lodger,resident,invitee
n|customer,client,buyer,shopper,consumer,patron,purchaser,user,regular,punter,prospect
n|client,customer,patron,consumer,buyer,user,applicant,patient,shopper
n|market,marketplace,bazaar,mart,fair,shop,exchange,trade,demand,outlet,sale,store,sell,retail
n|shop,store,boutique,outlet,supermarket,emporium,retailer,market,workshop,department store,deli,mart
n|trade,commerce,business,dealing,traffic,exchange,industry,occupation,craft,swap,profession,transaction,barter,sale
n|industry,business,commerce,manufacturing,production,trade,field,sector,effort,diligence,labour,hard work,activity
n|economy,financial system,wealth,thrift,resources,saving,prudence,frugality,efficiency,finances,market
n|government,administration,regime,state,authority,rule,leadership,ministry,cabinet,parliament,governance,control,executive
n|leader,head of state,ruler,premier,president,chief,monarch,sovereign,governor,commander
n|king,monarch,sovereign,ruler,emperor,crowned head,lord,majesty,prince,royalty
n|army,military,armed forces,troops,soldiers,forces,legion,militia,host,infantry,throng,multitude
n|soldier,fighter,warrior,trooper,serviceman,private,combatant,recruit,infantryman,veteran,man-at-arms
n|police,law enforcement,constabulary,cops,force,the law,officers,bobbies,detectives,patrol
n|crime,offence,felony,misdemeanour,wrongdoing,violation,illegality,misdeed,delinquency,lawbreaking,transgression,sin
n|court,tribunal,law court,bench,bar,courtyard,quadrangle,yard,retinue,entourage,royal household,woo,pitch
n|judge,magistrate,justice,referee,arbitrator,umpire,adjudicator,assessor,critic,connoisseur,authority,bench
n|justice,fairness,equity,impartiality,fair play,right,legality,lawfulness,judge,magistrate,righteousness,integrity
n|rights,entitlements,claims,freedoms,liberties,privileges,prerogatives
n|health,fitness,well-being,condition,shape,strength,vigour,constitution,form,state,wellness,soundness
n|illness,disease,sickness,ailment,disorder,complaint,malady,infection,condition,affliction,bug,infirmity
n|disease,illness,sickness,disorder,ailment,infection,condition,epidemic,complaint,plague,malady,virus,contagion
n|medicine,drug,medication,remedy,treatment,cure,pill,tablet,physic,prescription,tonic,healing,doctoring
n|doctor,physician,medic,gp,surgeon,consultant,practitioner,specialist,clinician,medical practitioner,quack
n|hospital,clinic,infirmary,sanatorium,medical centre,ward,health centre,sickbay,hospice
n|body,physique,figure,frame,form,build,torso,trunk,corpse,cadaver,mass,group,organisation,main part,collection,bulk
n|head,skull,crown,brain,mind,leader,chief,boss,top,front,summit,source,director,principal,intellect
n|hand,palm,fist,paw,mitt,worker,labourer,operative,applause,ovation,help,pointer,handwriting,script
n|face,countenance,features,visage,expression,look,front,surface,appearance,facade,dial,aspect,mug,side
n|eye,eyeball,orb,peeper,gaze,vision,sight,look,view,perception,appreciation,watch,centre
n|heart,core,centre,middle,soul,nucleus,courage,spirit,emotion,feeling,love,kernel,essence,crux,nub,pith,ticker
n|mind,brain,intellect,intelligence,head,reason,thoughts,wits,mentality,psyche,sense,understanding,memory,attention
n|brain,mind,intellect,intelligence,head,grey matter,cerebrum,wits,genius,scholar,thinker,egghead
n|life,existence,being,living,survival,vitality,animation,energy,biography,lifetime,activity,spirit,the world,span
n|death,dying,demise,end,passing,decease,loss,expiry,fatality,extinction,grave,destruction,mortality
n|moment,instance,juncture
n|sign,symbol,mark,signal,indication,gesture,omen,token,symptom,evidence,clue,notice,placard,hint,board
n|signal,sign,indication,gesture,cue,beacon,flag,alarm,warning,mark,light,motion,symbol,token,transmission
n|symbol,sign,emblem,token,badge,logo,representation,icon,mark,figure,character,image,type,metaphor
n|mark,spot,stain,blemish,smudge,line,sign,symbol,indication,grade,score,target,point,imprint,scar,label,note,emblem
n|line,row,stripe,streak,band,rope,cord,string,wire,queue,file,column,course,route,boundary,limit,wrinkle,furrow,outline,occupation
n|row,line,series,tier,rank,file,queue,column,bank,string,chain,argument,quarrel,dispute,tiff,racket,rumpus
n|circle,ring,round,disc,loop,hoop,sphere,globe,band,cycle,orbit,group,set,clique,coterie
n|square,quadrangle,plaza,piazza,block,four-sided,conservative,traditionalist,fogey,fair,honest,settle
n|colour,hue,shade,tint,tone,pigment,dye,paint,tinge,wash,complexion,vividness,richness,interest
n|light,illumination,brightness,glow,radiance,lamp,beam,ray,shine,daylight,sunshine,day,flame,lantern,lighter,spark,window
n|sound,noise,tone,din,racket,resonance,voice,tune,report,note,audio,clamour,hubbub,volume,pitch
n|noise,sound,din,racket,row,clamour,commotion,hubbub,tumult,cacophony,hullabaloo,uproar,babel,rumpus
n|voice,speech,tone,sound,utterance,expression,say,vote,opinion,words,articulation,organ,mouthpiece
n|music,melody,tune,harmony,song,score,strains,air,notes,composition,sound,rhythm,airs,ditty
n|song,tune,melody,ballad,anthem,number,air,chant,hymn,lay,carol,aria,track,ditty,warble,birdsong
n|film,movie,picture,motion picture,feature,flick,video,footage,coating,layer,membrane,skin,sheet,cinema,veneer
n|game,sport,pastime,match,contest,competition,fixture,amusement,activity,play,diversion,entertainment,recreation,quarry,wildlife,tie,round,plan
n|sport,game,exercise,athletics,pastime,physical activity,recreation,fun,amusement,joking,good sport,hobby,play
n|race,competition,contest,rally,chase,run,dash,sprint,breed,ethnic group,people,nation,stock,lineage,tribe,hurry
n|team,side,squad,line-up,club,company,crew,unit,band,group,pair,span,outfit
n|player,competitor,contestant,participant,athlete,sportsman,musician,performer,actor,team member,entrant,gambler
n|winner,victor,champion,conqueror,champ,prizewinner,first,medallist,hit,success,vanquisher,master,title-holder
n|prize,award,reward,trophy,medal,cup,honour,winnings,jackpot,purse,premium,accolade,laurels,booty
n|award,prize,trophy,medal,honour,reward,decoration,accolade,grant,gift,scholarship,bestowal,gong,commendation
n|gift,present,donation,offering,contribution,bonus,legacy,talent,flair,aptitude,knack,genius,bequest,endowment,grant,souvenir
n|party,celebration,festival,festivity,gala,jamboree,reception,gathering,fete,function,knees-up,do,shindig
n|holiday,vacation,break,leave,time off,trip,getaway,recess,rest,furlough,festival,day off,celebration,tour
n|weekend,saturday and sunday,days off,time off,break
n|morning,dawn,daybreak,sunrise,forenoon,a.m.,early hours,first light,cockcrow,morn,sunup
n|evening,dusk,nightfall,twilight,sunset,eve,sundown,eventide,gloaming,close of day,p.m.
n|night,darkness,dark,nighttime,dead of night,hours of darkness,evening,midnight,nightfall,small hours
n|summer,hot season,summertime,dog days,heyday,prime
n|winter,cold season,wintertime,frosty season,hibernal,midwinter,wintry period
n|spring,springtime,season of growth,source,fountain,wellspring,leap,bound,vault,coil,elasticity,bounce,jump,originate,stem
n|autumn,fall,harvest time,season of mists,decline,twilight years
n|story,floor,level,tier,storey,deck,landing
n|wall,barrier,partition,fence,screen,enclosure,rampart,barricade,bulwark,divider,panel,obstacle,fortification
n|door,entrance,entry,doorway,exit,gate,portal,opening,access,way in,hatch,hatchway
n|window,pane,opening,casement,skylight,glass,porthole,opportunity,opening,bay
n|floor,ground,deck,flooring,storey,level,tier,base,bottom,minimum,story,surface
n|roof,ceiling,top,canopy,covering,cover,peak,tiles,thatch,shelter,rooftop,summit
n|kitchen,galley,cookhouse,scullery,cooking area,kitchenette,canteen
n|garden,yard,plot,lawn,patch,park,allotment,grounds,bed,backyard,greenery
n|street,road,avenue,lane,way,drive,thoroughfare,boulevard,row,close,terrace,crescent,highway
n|bridge,span,viaduct,overpass,arch,link,connection,bond,flyover,crossing,causeway,tie
n|station,stop,terminal,depot,halt,base,post,office,headquarters,position,rank,channel,standing,location
n|airport,aerodrome,airfield,airstrip,terminal,landing strip,heliport,air base
n|energy,fuel,power,electricity,gas,petrol,oil,coal
n|piece of work,assignment,job,project,task,exercise,essay,paper
n|equipment,gear,apparatus,tools,kit,materials,supplies,machinery,paraphernalia,rig,outfit,stuff,hardware
n|supply,stock,store,reserve,provision,supplies,fund,hoard,cache,quantity,source,inventory,reservoir,stockpile,delivery
n|resource,asset,supply,source,reserve,aid,recourse,means,facility,reserves,materials,funds,ingenuity
n|effort,work,labour
n|service,help,assistance,use,duty,employment,ceremony,rite,maintenance,check,overhaul,worship,business,utility,benefit
n|use,usage,application,employment,exercise,utility,purpose,point,value,function,advantage,service,benefit,practice
n|usage,use,handling,treatment,employment,practice,custom,convention,tradition,wording,idiom,application,management
n|function,purpose,role,job,use,task,duty,activity,operation,responsibility,occasion,event,party,work,go,run,operate
n|operation,working,action,performance,functioning,running,process,procedure,surgery,campaign,manoeuvre,activity,business
n|action,act,deed,activity,move,step,measure,behaviour,effort,work,performance,lawsuit,battle,fighting,motion,exploit
n|act,deed,action,feat,move,measure,law,statute,bill,performance,routine,pretence,turn,exploit
n|activity,action,movement,exercise,pursuit,occupation,hobby,project,venture,bustle,liveliness,task,undertaking,business
n|behaviour,conduct,manners,actions,demeanour,ways,bearing,deportment,attitude,performance,operation,reaction
n|character,nature,personality,temperament,disposition,make-up,quality,integrity,honour,reputation,figure,role,symbol,letter,type,sort
n|personality,character,nature,temperament,disposition,make-up,charm,charisma,identity,celebrity,celebrity,star
n|reputation,name,standing,character,repute,status,fame,esteem,prestige,stature,renown,credit,honour,image
n|respect,esteem,regard,admiration,honour,reverence,deference,consideration,politeness,aspect,detail,way,point,sense,feature
n|honour,respect,integrity,esteem,glory,credit,distinction,fame,prestige,virtue,principles,privilege,award,tribute,decency
n|pride,self-respect,dignity,self-esteem,conceit,arrogance,vanity,satisfaction,pleasure,glory,ego,joy,honour
n|shame,embarrassment,humiliation,disgrace,dishonour,guilt,remorse,mortification,scandal,pity,stigma,ignominy
n|guilt,culpability,blame,fault,responsibility,shame,remorse,regret,contrition,sin,wrongdoing,conscience
n|trust,confidence,faith,belief,reliance,credit,certainty,hope,expectation,custody,care,responsibility,charge,fund
n|faith,belief,trust,confidence,religion,creed,conviction,loyalty,devotion,church,dogma,credence,reliance
n|belief,faith,conviction,opinion,view,trust,creed,notion,tenet,idea,principle,doctrine,persuasion,confidence,credence
n|religion,faith,belief,creed,church,worship,theology,cult,denomination,sect,devotion,persuasion,spirituality
n|god,deity,divinity,creator,lord,maker,supreme being,idol,spirit,goddess,almighty,divine being
n|soul,spirit,psyche,essence,heart,inner self,life force,being,person,individual,character,vitality,feeling,mind
n|spirit,soul,psyche,ghost,spectre,essence,heart,mood,courage,attitude,morale,vigour,energy,mettle,vitality,genie,gist,feeling
n|ghost,spirit,spectre,phantom,apparition,spook,wraith,shade,haunt,presence,trace,hint,suggestion,vestige
n|magic,sorcery,witchcraft,wizardry,enchantment,spell,conjuring,illusion,charm,allure,sleight of hand,glamour,occult
n|luck,fortune,chance,fate,destiny,serendipity,providence,fluke,success,break,accident,lot,windfall
n|fate,destiny,fortune,lot,providence,chance,luck,future,doom,kismet,karma,end,outcome,portion,predestination
n|truth,accuracy,validity,verity,authenticity
n|reality,actuality,truth,fact,certainty,real life,materiality,authenticity,existence,substance,physical world
n|appearance,look,looks,aspect,air,form,image,manner,expression,guise,arrival,emergence,advent,show,semblance,presence
n|look,appearance,glance,glimpse,gaze,peek,expression,air,aspect,style,fashion,mien,survey,inspection,observation,face
n|style,fashion,manner,way,mode,design,technique,approach,flair,elegance,class,chic,vogue,sort,type,kind,genre,form
n|fashion,style,trend,vogue,craze,mode,rage,fad,custom,look,design,method,way,manner,form,make,shape,mould
n|trend,tendency,movement,drift,direction,fashion,vogue,craze,fad,current,swing,inclination,flow,bias,course,leaning
n|tendency,inclination,leaning,trend,propensity,disposition,bent,bias,proneness,readiness,penchant,likelihood,drift
n|clothes,clothing,garments,attire,outfit,dress,apparel,wear,garb,costume,gear,wardrobe,togs,threads,kit
n|dress,frock,gown,garment,robe,clothes,clothing,attire,costume,outfit,garb,apparel,wear,suit,regalia
n|shoe,boot,trainer,sandal,slipper,footwear,pump,sneaker,loafer,plimsoll,clog
n|bag,sack,case,pouch,handbag,satchel,holdall,carrier,rucksack,purse,suitcase,briefcase,pack,basket,tote
n|box,case,carton,chest,crate,container,packet,trunk,casket,pack,coffer,bin,package,case,cubicle
n|bottle,flask,container,jar,vessel,phial,carafe,vial,decanter,flagon,demijohn
n|cup,mug,beaker,goblet,chalice,vessel,tumbler,trophy,glass,bowl,tankard,teacup
n|key,clue,answer,guide,solution,legend,pointer,opener,crucial,central,main,vital,pitch,tone
n|lock,fastener,bolt,latch,padlock,clasp,catch,secure,seal,tress,curl,strand,hair,fasten,bar
n|gate,entrance,door,barrier,exit,access,portal,opening,doorway,gateway,turnstile,wicket
n|fence,barrier,railing,hedge,wall,paling,rampart,palisade,stockade,barricade,enclosure
n|chair,seat,stool,bench,armchair,throne,pew,perch,professorship,presiding officer,head,leader,moderator,convene
n|bed,cot,bunk,couch,berth,divan,sack,hammock,mattress,resting place,base,foundation,plot,border,patch,layer,channel,bottom
n|picture frame,border,mount
n|fire,blaze,flames,inferno,conflagration,combustion,heat,burning,fervour,passion,spirit,sack,dismiss,shoot,discharge,launch,bombardment,barrage
n|smoke,fumes,vapour,gas,exhaust,smog,pollution,reek,puff,cloud,haze,cigarette,drag
n|storm,tempest,gale,squall,hurricane,cyclone,thunderstorm,downpour,blizzard,tornado,commotion,uproar,outburst,furore,rage,attack
n|rain,rainfall,drizzle,downpour,shower,precipitation,cloudburst,deluge,wet weather,storm,shower
n|wind,breeze,draught,gust,gale,blast,air current,breath,puff,flatulence,windiness,twist,turn,coil
n|snow,snowfall,flurry,blizzard,sleet,powder,slush,drift
n|ice,frost,permafrost,glacier,icicle,rime,iceberg,floe
n|sun,star,daylight,sunshine,sunlight,rays,sol,daystar,warmth
n|moon,satellite,lunar body,crescent,full moon,new moon,natural satellite
n|sky,heavens,firmament,atmosphere,air,blue,ether,vault,welkin,upper atmosphere,azure
n|earth,world,globe,planet,soil,ground,land,dirt,terra firma,sod,clay,loam,turf
n|ground,earth,soil,floor,land,terrain,turf,dirt,basis,reason,foundation,territory,field,area,pitch,sod,grounds,cause
n|land,ground,soil,earth,terrain,country,territory,property,estate,acres,realm,nation,region,countryside,domain,holding
n|field,meadow,pasture,paddock,lea,green,grassland,area,domain,sphere,subject,discipline,speciality,pitch,ground,arena,range
n|park,garden,green,grounds,woodland,parkland,common,playground,reserve,estate,recreation ground,leave,station,stop
n|beach,shore,coast,seashore,sands,seaside,strand,foreshore,seaboard,waterfront,lido
n|coast,shore,seaside,seaboard,coastline,beach,seashore,strand,littoral,glide,cruise,freewheel,drift
n|island,isle,islet,atoll,cay,key,archipelago,ait,holm
n|stone,rock,pebble,boulder,cobble,gem,jewel,gemstone,pit,seed,pip,kernel,rock,monument
n|rock,stone,boulder,crag,outcrop,pebble,cliff,sway,swing,roll,shake,shock,stagger,reel
n|metal,alloy,ore,mineral,iron,steel,copper,tin,element,bullion
n|gold,bullion,gilt,gold leaf,riches,wealth,treasure,yellow metal,aureate,golden
n|glass,pane,crystal,tumbler,goblet,mirror,lens,flute,beaker,window,glassware,looking glass
n|paper,sheet,page,stationery,tissue,parchment,card,newsprint,wallpaper,document
n|wood,timber,lumber,planks,logs,boards,forest,woodland,copse,thicket,trees,firewood,kindling
n|plastic,synthetic,polymer,resin,vinyl,artificial,man-made,flexible,pliable
n|cloth,fabric,material,textile,rag,stuff,duster,napkin,towel,drapery,web,weave
n|thread,fibre,strand,cotton,yarn,filament,string,twine,line,theme,drift,tenor,course,spiral
n|rope,cord,cable,line,string,twine,lasso,hawser,lariat,tether,strap
n|chain,series,sequence,string,succession,train,progression,set,cycle,fetters,shackles,manacles,restraint,bind,tether
n|net,mesh,web,netting,network,lattice,trap,snare,earn,clear,catch,bag,final,take-home,cobweb
n|wire,cable,lead,flex,cord,filament,telegram,strand,line,electrical cord
n|network,system,web,grid,net,complex,organisation,mesh,arrangement,structure,circuit,lattice,maze,labyrinth
n|computer,pc,laptop,machine,processor,desktop,workstation,device,notebook,mainframe,server
n|program,software,application,app,routine,code,script,schedule,plan,syllabus,curriculum,agenda,show,broadcast,programme
n|screen,display,monitor,partition,shield,barrier,divider,cover,shelter,curtain,window,projection surface,television
n|file,document,folder,record,dossier,case,portdocxtor,archive,column,line,row,rank,queue,binder,tool,rasp
n|page,sheet,leaf,docxtor,side,paper,attendant,pageboy,servant,call,summon,site,screen
n|letter,character,symbol,sign,epistle,note,message,missive,line,communication,document,dispatch,mail,literal
n|number,digit,figure,integer,numeral,total,count
n|code,cipher,system,rules,law,regulations,convention,script,programme,software,principle,ethics,symbols,cryptogram
n|language,tongue,speech,dialect,idiom,vocabulary,words,terminology,parlance,communication,jargon,vernacular,lingo,diction,expression
n|accent,pronunciation,intonation,inflection,tone,stress,emphasis,brogue,twang,burr,diacritic,mark,highlight
n|grammar,syntax,rules of language,language rules,morphology,usage,sentence structure,linguistics
n|vocabulary,words,lexicon,terminology,glossary,language,wordbook,dictionary,thesaurus,idiom,word stock,jargon
n|synonym,equivalent,alternative word,substitute,same,similar word,parallel,counterpart
n|dictionary,lexicon,glossary,wordbook,vocabulary,thesaurus,reference book,encyclopedia,concordance
n|chapter,section,part,division,episode,period,phase,stage,passage,segment,clause,topic,verse,instalment,era,branch
n|paragraph,section,passage,item,clause,part,portion,piece,subsection,notice,note
n|heading,title,headline,caption,rubric,legend,head,topic,banner,label,header,name,subtitle
n|list,register,table,index,catalogue,directory,agenda,schedule,tilt,lean,incline,slant,roster,roll,inventory
n|note,memo,message,record,jotting,letter,line,entry,comment,annotation,remark,footnote,mark,sign,tone,observation,banknote,heed,notice
n|reference,citation,mention,allusion,note,quotation,source,testimonial,character,recommendation,credentials,endorsement,link,regard
n|quotation,quote,citation,extract,passage,excerpt,reference,selection,estimate,price,tender,line,cutting,phrase
n|source,reference,authority,informant,witness,origin,supplier,wellspring
n|author,writer,novelist,creator,composer,originator,poet,penman,wordsmith,architect,columnist,essayist,founder,journalist
n|writer,author,novelist,scribe,columnist,poet,journalist,essayist,penman,wordsmith,correspondent,biographer,hack
n|reader,bookworm,scholar,audience,subscriber,lecturer,interpreter,primer,textbook,reciter
n|audience,spectators,viewers,listeners,crowd,gallery,house,congregation,public,turnout,assembly,readership,hearing,interview,meeting
n|crowd,throng,mob,multitude,horde,mass,host,gathering,swarm,herd,flock,crush,audience,gate,pack,army,drove
n|press,media,newspapers,journalists,reporters,fourth estate,papers,printing,publisher,newsmen,newspaper,push,squeeze,crush,iron,flatten
n|news,information,report,story,word,bulletin,headlines,intelligence,announcement,gossip,tidings,account,release,update,exposé,communiqué
n|story,account,anecdote,article,report,tale,narrative,yarn,saga,plot,history,fiction,lie,feature,legend
n|rumour,gossip,hearsay,talk,whisper,speculation,tittle-tattle,report,story,buzz,scandal,word,canard,tidings
n|fame,renown,celebrity,glory,stardom,reputation,prominence,popularity,eminence,note,notoriety,name,honour,distinction
n|attention,care,notice,mind
n|view,sight,outlook,prospect,panorama,landscape,scene,vista,scenery,aspect,perspective,survey,look,observation
n|scene,setting,location,place,site,spot,view,sight,landscape,vista,display,fuss,exhibition,incident,episode,part,section,locale,backdrop,tableau,world,sphere
n|landscape,scenery,countryside,view,vista,panorama,terrain,scene,outlook,setting,prospect,land,topography,geography
n|setting,background,location,backdrop,environment,scene,surroundings,context,site,milieu,mounting,frame,position,set,place,locale
n|context,circumstances,background,setting,situation,conditions,frame of reference,framework,environment,perspective,position
n|theme,subject,topic,idea,thesis,motif,thread,gist,keynote,burden,essay,matter,melody,tune,subject matter,leitmotif
n|topic,subject,theme,issue,matter,question,point,text,thesis,argument,thread,area,field,talking point,concern,business
n|subject,topic,theme,issue,matter,question,point,area,field,discipline,course,object,case,national,citizen,patient,participant,victim,expose,subordinate
n|content,subject matter,substance,material,text,ideas,matter,contents,meaning,burden,gist,amount,capacity,volume,load
n|contents,constituents,ingredients,parts,elements,components,load,matter,subject matter,topics,themes,chapters,divisions,index,table of contents,items,material
n|meeting,gathering,assembly,conference,session,get-together,encounter,rally,council,congress,appointment,convention,date,tryst,confluence,junction,union,forum,reunion
n|conference,meeting,convention,congress,seminar,forum,symposium,colloquium,assembly,council,discussion,consultation,summit,workshop
n|presentation,talk,speech,address,lecture,demonstration,performance,show,introduction,delivery,exhibition,display,staging,appearance,offering,award,bestowal
n|speech,address,talk,lecture,oration,discourse,presentation,monologue,sermon,harangue,declamation,communication,language,articulation,diction,utterance,dialect,tongue
n|lecture,talk,speech,address,discourse,presentation,oration,sermon,reprimand,telling-off,scolding,homily,teach,reprove,harangue
n|interview,meeting,audience,talk,dialogue,conversation,consultation,examination,evaluation,discussion,hearing,questioning,question,quiz
n|question,ask,query
n|request,appeal,demand,petition,application,plea,entreaty,call,suit,solicitation,requisition,desire,wish
n|offer,proposal,bid,proposition,suggestion,tender,approach,overture,submission,presentation,attempt,go,try,extension,deal,motion
n|proposal,plan,suggestion,proposition,scheme,offer,recommendation,bid,project,idea,motion,design,programme,tender,submission,overture
n|promise,pledge,vow,word,guarantee,assurance,oath,undertaking,commitment,bond,potential,talent,ability,flair,swear,assure
n|permission,consent,authorisation,approval,leave,sanction,go-ahead,licence,agreement,blessing,say-so,clearance,assent,permit,warrant,okay,green light,liberty,allowance
n|approval,consent,permission,agreement,sanction,go-ahead,blessing,acceptance,authorisation,support,admiration,endorsement,praise,favour,ratification
n|refusal,rejection,denial,no,rebuff,veto,turndown,dismissal,knockback,repudiation,declining,negation,decline
n|instruction,direction,order,command,guidance,teaching,advice,education,training,tuition,demand,orders,directive,recommendation,briefing,lesson
n|order,command,instruction,direction,directive,decree,mandate,ruling,sequence,arrangement,organisation,system,tidiness,discipline,calm,peace,request,booking,class,type,kind
n|command,order,instruction,directive,decree,control,power,authority,mastery,grasp,knowledge,charge,rule,domination,sway,leadership,dictate
n|rule,reign,government,control,sovereignty,command,dominion,sway,authority,jurisdiction,power,regime,leadership
n|authority,power,control,command,influence,jurisdiction,rule,dominion,expert,specialist,professional,permission,authorisation,right,sanction,government,officials,source
n|responsibility,duty,obligation,burden,charge,liability,accountability,blame,fault,onus,role,job,task,trust,care,authority,guilt
n|obligation,duty,responsibility,commitment,requirement,debt,burden,contract,charge,task,need,liability,bond,promise
n|requirement,need,demand,necessity,condition,stipulation,essential,prerequisite,specification,must,qualification,provision,precondition,term,want,lack
n|need,necessity,requirement,demand,want,lack,shortage,essential,call,obligation,poverty,distress,hardship,deprivation,want,urgency
n|lack,shortage,absence,want,deficiency,scarcity,deficit,paucity,dearth,need,insufficiency,void,gap,shortfall,deprivation,omission
n|shortage,lack,deficit,scarcity,deficiency,dearth,shortfall,paucity,want,insufficiency,absence,poverty,drought
n|excess,surplus,glut,surfeit,overkill,extra,superfluity,overabundance,plethora,remainder,leftover,overflow,overindulgence,intemperance
n|abundance,plenty,wealth,profusion,riches,affluence,cornucopia,flood,glut,fullness,bounty,lavishness,multitude
n|variety,diversity,range,assortment,selection,mixture,medley,array,collection,mix,choice,change,difference,type,kind,sort,breed,strain,class,brand
n|diversity,variety,range,assortment,heterogeneity,multiplicity,difference,distinctiveness,breadth,mixture,medley,variation
n|mixture,blend,combination,mix,compound,fusion,amalgam,medley,assortment,cocktail,union,brew,alloy,variety,jumble,composite,miscellany,hybrid
n|combination,mixture,blend,union,merger,amalgam,compound,fusion,integration,synthesis,alliance,coalition,association,partnership,coupling,joining,link
n|collection,accumulation,anthology,compilation,store,stockpile,hoard,pile,heap,assortment,gathering,set,series
n|balance,equilibrium,stability,poise,steadiness,parity,symmetry,equality,evenness,remainder,rest,surplus,difference,counterbalance,equivalence,offset,weigh
n|harmony,agreement,accord,unity,peace,concord,cooperation,tune,melody,balance,sympathy,consistency,rapport,symmetry,order
n|unity,union,oneness,togetherness,solidarity,harmony,agreement,accord,uniformity,integrity,wholeness,concord,cohesion,identity
n|cooperation,collaboration,teamwork,partnership,assistance,help,support,coordination,unity,combined effort,give-and-take,joint action
n|competition,contest,rivalry,race,tournament,match,championship,game,event,opposition,struggle,opponents,competitors,fight,challenge,contention
n|opposition,resistance,hostility,antagonism,objection,rivals,opponents,enemy,competition,disapproval,dissent,contrast,counterbalance
n|pressure,coercion,compulsion,influence,constraint
n|support,evidence,backing,prop
n|improvement,upgrade,better,enhancement
n|decline,fall,drop,decrease,reduction,downturn,slump,recession,deterioration,dwindling,weakening,lessening,downswing,wane,diminution
n|increase,rise,growth,gain,boost,expansion,upsurge,jump,escalation,increment,addition,upturn,hike,swelling,augmentation,surge
n|reduction,cut,decrease,decline,lessening,fall,drop,cutback,discount,shrinkage,curtailment,deduction,abatement,contraction,diminution,downturn
n|rise,increase,growth,climb,ascent,upturn,upsurge,hike,boost,upswing,incline,slope,advancement,promotion,elevation
n|fall,drop,decline,decrease,collapse,tumble,plunge,slump,descent,dip,downturn,reduction,spill,cut,defeat,ruin,surrender,downfall,autumn
n|jump,leap,bound,spring,vault,hop,skip,rise,increase,hurdle,fence,gap,hike,surge,start,jolt,startle,bounce
n|movement,motion,action,activity,gesture,move,shift,migration,campaign,crusade,group,organisation,faction,trend,development,progress,transfer,passage,flow
n|motion,movement,action,gesture,move,progress,passage,flow,proposal,proposition,suggestion,signal,gesticulation,wave,sign,mobility
n|speed,pace,rate,velocity,tempo,quickness,swiftness,rapidity,haste,hurry,momentum,celerity,rush,race,dash,belt,accelerate
n|direction,course,route,way,path,bearing,line,heading,tendency,trend,orientation,management,control,guidance,leadership,instruction,command,supervision,order,aim,drift
n|distance,length,space,gap,extent,range,interval,span,stretch,reach,remoteness,detachment,reserve,aloofness,coolness,separation,mileage
n|length,extent,distance,span,measure,reach,duration,term,period,piece,portion,stretch,section,segment,longness,linear extent
n|height,altitude,elevation,tallness,stature,loftiness,peak,summit,high point,apex,highness,top,zenith,hill,eminence
n|depth,deepness,profundity,drop,extent,intensity,seriousness,wisdom,insight,penetration,richness,gravity,vividness,abyss,pit,thickness
n|width,breadth,span,thickness,wideness,extent,scope,diameter,range,girth,beam,compass,broadness,spread
n|weight,heaviness,mass,load,burden,pressure,importance,significance,influence,force,substance,ballast,tonnage,poundage,value,load,onus,clout
n|temperature,heat,warmth,degrees,fever,coldness,hotness,thermal reading
n|heat,warmth,hotness,high temperature,fever,passion,fervour,intensity,excitement,pressure,fire,enthusiasm,ardour,warm up,cook,reheat
n|cold,chill,coldness,frost,iciness,coolness,freeze,bitterness,flu,virus,bug,cough,sneezes,catarrh,nippiness
n|air,atmosphere,sky,oxygen,breeze,wind,draught,aura,appearance,manner,look,mood,feeling,tune,melody,song,broadcast,voice,express
n|gas,vapour,fumes,fuel,petrol,natural gas,hot air,flatulence,empty talk
n|liquid,fluid,solution,juice,sap,drink,water,flow,watery,molten,runny
n|solid,hard,firm,dense,compact,block,material,rigid,sturdy,real,substantial
n|mass,bulk,body,lump,heap,pile,crowd,throng,multitude,majority,size,magnitude,weight,whole,stack,block,chunk,public,people
n|volume,quantity,amount,capacity,bulk,size,dimensions,book,tome,loudness,sound level,mass,extent,measure,load
n|capacity,volume,size,room,space,dimensions,ability,power,potential,faculty,role,function,position,competence,talent,aptitude,range
n|space,gap,area,room,expanse,interval,distance,blank,opening,vacancy,void,cosmos,universe,outer space,extent,scope,time
n|reserve,store,stock,supply,fund,hoard,stockpile,savings,backup,restraint,shyness,reticence,substitute,book,hold,keep,retain,park,sanctuary
n|stock,supply,store,reserve,inventory,goods,merchandise,range,hoard,cache,capital,shares,livestock,ancestry,breed,sauce,broth,standard,regular,conventional
n|chance,likelihood,probability,odds,possibility,prospect,hazard,gamble,luck,coincidence
n|probability,likelihood,chance,odds,prospect,possibility,expectation,feasibility
n|certainty,sureness,confidence,assurance,conviction,fact,truth,guarantee,inevitability,reality,security,positiveness
n|doubt,uncertainty,question,hesitation,suspicion,misgiving,mistrust,scepticism,reservation,distrust,disbelief,ambiguity,confusion,suspect,query
n|belief,suspicion,trust
n|confidence,assurance,self-assurance,poise,faith,trust,belief,certainty,reliance,secret,boldness,nerve,aplomb,self-belief
n|fear,anxiety,concern,disquiet
n|attraction,appeal,charm,draw,pull,allure,lure,magnetism,fascination,enticement,temptation,interest,drawing power,pleasure,sight
n|beauty,loveliness,attractiveness,prettiness,elegance,charm,glamour,grace,allure,splendour,good looks,belle,stunner,advantage,asset,virtue
n|ugliness,unattractiveness,plainness,hideousness,unsightliness,repulsiveness
n|art,painting,drawing,sculpture,craft,skill,artistry,technique,knack,flair,creativity,fine art,artwork,cunning,craftiness,trickery,wile
n|artist,painter,sculptor,illustrator,creator,craftsman,master,draughtsman,artiste,designer,performer,virtuoso,expert
n|culture,arts,civilisation,refinement,education,sophistication,development,polish,breeding,taste,cultivation
n|literature,writings,books,works,texts,publications,letters,poetry,fiction,prose,printed matter,leaflets,brochures,information,papers,research
n|poem,verse,rhyme,ode,sonnet,lyric,ballad,poetry,rhyming piece,limerick,epic,song,stanza
n|play,drama,show,performance,production,piece,comedy,tragedy,stage work,theatre,farce,pantomime,fun,recreation,leisure,sport,movement,freedom,give,slack,looseness
n|show,display,exhibition,presentation,performance,production,programme,spectacle,array,expo,fair,demonstration,pretence,appearance,front,pageant,parade,broadcast
n|performance,show,presentation,production,act,appearance,concert,gig,rendition,portrayal,acting,interpretation,functioning,operation,running,efficiency,conduct,fuss
n|exhibition,display,show,presentation,fair,exposition,demonstration,spectacle,array,showing,expo,showcase
n|attraction,tourist spot,sight,landmark,draw,feature,highlight,point of interest
n|entertainment,amusement,leisure,diversion,fun,recreation,enjoyment,distraction,show,performance,pleasure,sport,pastime,hospitality,relaxation,play
n|leisure,free time,spare time,rest,relaxation,recreation,time off,holiday,ease,break,idle time,freedom,convenience
n|hobby,pastime,interest,diversion,pursuit,sideline,recreation,leisure activity,relaxation,amusement,craft,avocation
n|health care,medicine,medical care,treatment,healthcare,welfare
n|diet,food,fare,nutrition,regimen,fast,slimming plan,eating plan,sustenance,restriction,weight-loss plan,abstinence
n|exercise,workout,training,activity,physical activity,effort,exertion,drill,practice,work,use,task,problem,lesson,assignment,application,employment
n|practice,custom,habit,routine,tradition,convention,rule,training,rehearsal,drill,exercise,preparation,business,work,profession,application,use,way
n|rest,break,relaxation,pause,respite,breather,leisure,interval,holiday,repose,remainder,others,balance,remnant,support,stand,sleep,nap
n|sleep,rest,slumber,doze,nap,siesta,repose,snooze,kip,forty winks,hibernation,shut-eye,catnap,dormancy,trance
n|dream,nightmare,reverie,fantasy,vision,hallucination,trance,illusion,daydream,delight,beauty,joy,ideal
n|memory,recollection,remembrance,recall,reminiscence,retention,mind,nostalgia,souvenir,commemoration,storage,capacity,impression,image
n|mind,thought,attention,care,memory,intention,opinion
n|attention,focus,concentration,heed,awareness
n|skill,expertise,competence,mastery,prowess,facility,aptitude,know-how,proficiency,adroitness,accomplishment,dexterity,savoir faire
n|experience,knowledge,practice,know-how,background,history,involvement,expertise,skill,training,exposure,event,incident,occurrence,adventure,episode,ordeal,encounter
n|practice,training
n|difference,contrast,variation
n|crisis,emergency,predicament,catastrophe
n|solution,answer,key,fix
n|success,achievement
n|opportunity,break,chance,possibility,moment
n|trouble,difficulty
r|quickly,fast,rapidly,swiftly,speedily,promptly,briskly,hastily,hurriedly,at speed,apace,expeditiously
r|slowly,gradually,leisurely,unhurriedly,steadily,sluggishly,at a snail's pace,little by little,ploddingly,gently
r|very,extremely,really,highly,exceedingly,remarkably,especially,particularly,truly,awfully,exceptionally,immensely,terribly,decidedly,most
r|extremely,exceedingly,exceptionally,tremendously,immensely,intensely,enormously,remarkably,incredibly,awfully,very,unusually
r|really,truly,genuinely,actually,indeed,in fact,certainly,honestly,absolutely,positively,seriously,very
r|quite,rather,fairly,somewhat,moderately,reasonably,pretty,relatively,a bit,comparatively,fully,wholly,completely,entirely
r|rather,quite,fairly,somewhat,pretty,moderately,slightly,a little,reasonably,relatively,preferably,instead,sooner
r|almost,nearly,virtually,practically,all but,just about,roughly,approximately,close to,well-nigh,as good as,nigh,not quite
r|nearly,almost,virtually,practically,all but,just about,approximately,close to,roughly
r|only,just,merely,simply,solely,purely,exclusively,barely,hardly,entirely,alone,but,no more than,at most
r|also,too,as well,in addition,besides,additionally,moreover,furthermore,likewise,plus,along with,to boot,what is more
r|moreover,furthermore,in addition,besides,also,what is more,additionally,likewise,further,on top of that,to boot
r|however,but,yet,nevertheless,nonetheless,still,though,even so,all the same,on the other hand,notwithstanding,despite that,mind you
r|therefore,thus,so,consequently,hence,accordingly,as a result,for that reason,ergo,that being so,then
r|thus,therefore,so,hence,consequently,accordingly,in this way,like this,in this manner,as a result,ergo
r|often,frequently,regularly,repeatedly,commonly,many times,usually,generally,time and again,over and over,again and again,habitually,oft
r|sometimes,occasionally,now and then,at times,from time to time,once in a while,periodically,on occasion,every so often,intermittently
r|always,constantly,continually,invariably,forever,perpetually,consistently,every time,at all times,eternally,ever,without fail,unfailingly
r|never,not ever,at no time,not at all,on no account,under no circumstances,no way,not once,nevermore,not in the least
r|usually,normally,generally,typically,commonly,ordinarily,customarily,habitually,as a rule,mainly,mostly,regularly,by and large
r|generally,usually,mostly,mainly,largely,on the whole,broadly,commonly,typically,as a rule,in general,by and large,overall
r|mostly,mainly,largely,chiefly,principally,primarily,predominantly,generally,for the most part,on the whole,in the main,usually
r|mainly,chiefly,primarily,principally,largely,mostly,predominantly,in the main,for the most part,especially,above all
r|especially,particularly,specially,notably,chiefly,mainly,expressly,in particular,principally,above all,exceptionally,markedly
r|particularly,especially,specifically,in particular,notably,expressly,exceptionally,remarkably,uncommonly,distinctly
r|probably,likely,presumably,doubtless,most likely,in all likelihood,no doubt,as like as not,perhaps,seemingly,apparently,reasonably
r|perhaps,maybe,possibly,conceivably,perchance,it may be,for all one knows,feasibly,peradventure,it could be
r|possibly,perhaps,maybe,conceivably,feasibly,potentially,perchance,at all,by any chance,in any way
r|certainly,definitely,surely,undoubtedly,absolutely,without doubt,of course,indeed,positively,assuredly,unquestionably,clearly,naturally,by all means
r|definitely,certainly,surely,absolutely,positively,unquestionably,undoubtedly,categorically,without doubt,decidedly,clearly,distinctly
r|clearly,obviously,plainly,evidently,apparently,patently,manifestly,undoubtedly,distinctly,visibly,lucidly,transparently,unmistakably
r|obviously,clearly,plainly,evidently,patently,manifestly,undeniably,of course,naturally,needless to say,unmistakably,apparently
r|apparently,seemingly,evidently,ostensibly,outwardly,on the face of it,it seems,supposedly,reportedly,superficially,allegedly
r|actually,really,in fact,in reality,truly,as a matter of fact,indeed,literally,genuinely,in truth,as it happens
r|indeed,in fact,really,actually,certainly,truly,undeniably,in truth,in reality,admittedly,absolutely
r|exactly,precisely,accurately,just,correctly,specifically,literally,strictly,to the letter,absolutely,quite,faithfully,right
r|precisely,exactly,accurately,just,specifically,strictly,expressly,literally,definitely,absolutely,to the letter
r|approximately,roughly,about,around,nearly,close to,circa,more or less,or so,in the region of,some,loosely,almost,give or take
r|together,jointly,collectively,as one,in unison,side by side,in concert,hand in hand,at once,simultaneously,cooperatively,mutually
r|separately,individually,apart,independently,singly,one by one,alone,severally,distinctly,respectively,on one's own,solo
r|alone,solo,by oneself,independently,unaccompanied,on one's own,singly,unaided,solely,only
r|completely,totally,entirely,fully,wholly,absolutely,utterly,thoroughly,altogether,perfectly,quite,in full,downright,outright,through and through
r|totally,completely,entirely,utterly,wholly,fully,absolutely,thoroughly,altogether,perfectly,downright,unreservedly
r|partly,partially,in part,somewhat,to some extent,slightly,up to a point,in some measure,incompletely,halfway,to a degree
r|slightly,a little,somewhat,a bit,marginally,faintly,barely,hardly,lightly,minimally,to a small extent,imperceptibly,mildly
r|barely,hardly,scarcely,only just,just,narrowly,by a whisker,by the skin of one's teeth,faintly,almost not,at a push
r|hardly,barely,scarcely,only just,not quite,by no means,not at all,rarely,seldom
r|rarely,seldom,infrequently,hardly ever,scarcely ever,occasionally,not often,once in a blue moon,little,uncommonly
r|soon,shortly,presently,before long,in a while,anon,in a minute,in the near future,any minute now,imminently,promptly,quickly
r|immediately,at once,instantly,right away,straight away,directly,promptly,forthwith,without delay,now,pronto,instantaneously
r|now,at present,currently,at the moment,nowadays,presently,right now,today,these days,immediately,at once,just now,already
r|currently,at present,presently,now,at the moment,nowadays,these days,at this time,right now,today,so far
r|recently,lately,not long ago,newly,of late,freshly,latterly,in recent times,just now,a short time ago
r|early,ahead of time,in good time,beforehand,previously,prematurely,betimes,soon,at dawn,before time,sooner
r|late,belatedly,behind schedule,tardily,overdue,at the last minute,after time,behind time,lately,formerly,deceased
r|before,previously,earlier,beforehand,formerly,sooner,prior to,ahead,in advance,already,once,until now,ere
r|after,later,afterwards,subsequently,following,next,then,thereafter,behind,in the wake of,since,at the end of,past
r|later,afterwards,subsequently,after,next,thereafter,then,in time,eventually,at a later date,by and by,in due course
r|finally,at last,eventually,in the end,ultimately,lastly,in conclusion,at length,after all,in time,last of all,once and for all
r|eventually,ultimately,finally,in the end,at last,after all,in time,sooner or later,in the long run,some day,one day,at length
r|suddenly,abruptly,all of a sudden,unexpectedly,without warning,out of the blue,swiftly,instantly,sharply,hastily,in a flash,on the spur of the moment
r|gradually,slowly,little by little,step by step,progressively,bit by bit,steadily,by degrees,gently,piecemeal,evenly,imperceptibly
r|again,once more,once again,anew,afresh,another time,over again,repeatedly,also,further,yet again,encore,after all
r|already,previously,before,by now,by this time,even now,so soon,as yet,beforehand,at an earlier time,by then,yet
r|still,yet,even now,even then,nevertheless,nonetheless,however,all the same,even so,but,in any case,motionless,calm,quiet,tranquil,stationary,silent
r|yet,still,so far,until now,thus far,up to now,even,already,besides,however,nevertheless,eventually,as well,but
r|ever,always,at any time,at all times,at any point,on any occasion,continually,forever,constantly,in any case,eternally,perpetually
r|just,only,merely,simply,barely,exactly,precisely,recently,lately,fair,fairly,justly,rightly,equitably,honestly,honourably
r|even,still,yet,all the more,indeed,evenly,smoothly,flat,level,regularly,steadily,equal,equitable,fair,balanced,uniform,tied,drawn
r|well,effectively,satisfactorily,skilfully,ably,thoroughly,properly,fully,nicely,successfully,adequately,sufficiently,healthy,fit,sound
r|badly,poorly,wrongly,inadequately,incorrectly,unsatisfactorily,ineptly,inexpertly,imperfectly,defectively,seriously,severely,sorely,greatly
r|carefully,cautiously,thoroughly,meticulously,attentively,diligently,warily,prudently,painstakingly,scrupulously,conscientiously,gingerly,closely
r|easily,effortlessly,simply,readily,smoothly,without difficulty,with ease,comfortably,by far,undoubtedly,hands down,without a hitch
r|hard,intensely,forcefully,strenuously,vigorously,diligently,energetically,earnestly,keenly,heavily,severely,violently,powerfully
r|directly,straight,immediately,right,exactly,candidly,frankly,openly,plainly,without delay,at once,promptly,personally,face to face
r|frankly,candidly,honestly,openly,plainly,bluntly,truthfully,sincerely,directly,straightforwardly,to be honest,to be frank,in truth
r|honestly,truthfully,sincerely,genuinely,candidly,frankly,legitimately,justly,fairly,really,truly,openly,in good faith,upfront
r|simply,just,merely,only,plainly,easily,straightforwardly,clearly,basically,purely,solely,completely,absolutely,modestly,naturally
r|basically,essentially,fundamentally,in essence,primarily,at heart,mainly,at bottom,in principle,chiefly,inherently,intrinsically
r|essentially,basically,fundamentally,in essence,at heart,at bottom,primarily,in principle,intrinsically,inherently,substantially,mainly
r|in addition,also,as well,besides,furthermore,moreover,further,too,to boot,plus,on top of that,what's more,additionally
r|for example,for instance,such as,say,namely,as an illustration,to illustrate,e.g.,like,in particular,by way of example,to give an example
r|in conclusion,to conclude,finally,in summary,in short,to sum up,lastly,in closing,all in all,on the whole,in brief,to summarise,in a nutshell
r|on the other hand,conversely,in contrast,by contrast,alternatively,then again,instead,on the contrary,but,whereas,at the same time
r|in contrast,by contrast,conversely,on the other hand,on the contrary,whereas,while,unlike,but,however,alternatively,instead
r|as a result,consequently,therefore,thus,hence,so,accordingly,for this reason,because of this,as a consequence,in consequence,ergo
r|in fact,actually,indeed,as a matter of fact,in reality,really,truly,in truth,to tell the truth,in point of fact,well
r|for this reason,therefore,thus,consequently,hence,accordingly,so,on that account,that's why,as a result
r|despite,in spite of,notwithstanding,regardless of,even with,against,in the face of,undeterred by,for all
r|although,though,even though,while,whereas,albeit,despite the fact that,notwithstanding,granted that,even if,much as
r|because,since,as,for,owing to the fact that,on account of,in view of the fact that,seeing that,given that,thanks to,by reason of
r|so,therefore,thus,consequently,as a result,hence,accordingly,very,extremely,really,thus far,like this,that much,in order that
r|too,also,as well,in addition,besides,furthermore,excessively,overly,unduly,over,extremely,unreasonably,immoderately,inordinately
r|enough,sufficiently,adequately,amply,satisfactorily,reasonably,fairly,tolerably,passably,abundantly,plenty,quite
r|far,much,a great deal,considerably,greatly,significantly,markedly,by a long way,remotely,afar,distantly,a long way,miles
r|much,greatly,a lot,considerably,often,frequently,significantly,markedly,extensively,substantially,regularly,appreciably,to a great extent
r|more,further,additionally,extra,longer,again,better,over,beyond,besides,to a greater extent,in addition
r|less,fewer,not as much,smaller,lower,reduced,minus,not so,slighter,to a lower degree,lesser
r|nowhere,not anywhere,no place,not in any place,in no place,not a single place
r|everywhere,all over,throughout,far and wide,here and there,high and low,all around,ubiquitously,universally,in every place,pervasively
r|here,in this place,at this spot,present,hither,in this spot,at hand,on this spot,right here,now
r|there,in that place,at that spot,yonder,thither,over there,in that spot,thereabouts,at that point,in that respect
r|away,off,absent,elsewhere,gone,out,abroad,at a distance,afar,aside,far,not at home,continuously,forth
r|nearby,close,near,at hand,close by,not far away,within reach,adjacent,handy,neighbouring,hard by,around
r|abroad,overseas,in foreign lands,out of the country,in a foreign country,away,elsewhere,out and about,widely,at large
r|inside,indoors,within,in,interior,internally,in the house,under cover,on the inside,in the interior
r|outside,outdoors,out of doors,outdoor,externally,in the open air,out,alfresco,in the open,on the outside,beyond,exterior
r|above,overhead,up,on high,over,aloft,higher,earlier,previously,upstairs,beyond,at a higher level,atop,upward
r|below,beneath,under,underneath,lower,down,further down,downstairs,less than,at a lower level,after,later,infra
r|around,about,round,all over,near,approximately,roughly,nearby,here and there,on all sides,in the vicinity,circa,in circulation
r|across,over,crosswise,from side to side,transversely,athwart,crossways,through,beyond,on the other side,to the far side
r|along,forward,onward,ahead,on,together,with,alongside,beside,by the side of,lengthwise,in company
r|away from,off,from,out of,aside from,absent,distant from
r|apart,separately,aside,asunder,to pieces,away,independently,distant,to one side,in bits,in two,individually
r|together with,along with,as well as,plus,coupled with,accompanied by,in addition to,with
r|instead,alternatively,rather,in lieu,as a substitute,as an alternative,in place,on the other hand,preferably,in preference,in its place
r|otherwise,or else,if not,or,in other respects,differently,in any other way,in other ways,under other circumstances,contrarily,apart from that
r|likewise,similarly,also,too,in the same way,equally,in like manner,by the same token,the same,ditto,correspondingly
r|similarly,likewise,in the same way,equally,in a similar manner,in like manner,by the same token,correspondingly,comparably,analogously
r|differently,otherwise,in another way,contrarily,variously,diversely,distinctively,unconventionally,uniquely,separately,disparately
r|particularly,exceptionally,remarkably,notably,uncommonly,peculiarly,singularly
r|naturally,of course,obviously,certainly,as one would expect,needless to say,normally,instinctively,spontaneously,by nature,innately,unaffectedly
r|fortunately,luckily,happily,thankfully,by good luck,as luck would have it,mercifully,providentially,auspiciously,opportunely
r|unfortunately,sadly,regrettably,unhappily,alas,unluckily,sad to say,lamentably,woefully,as ill luck would have it,to one's regret
r|certainly,surely
r|perhaps,possibly
r|openly,publicly,frankly,candidly,overtly,plainly,blatantly,flagrantly,unashamedly,in public,brazenly,unreservedly,transparently
r|secretly,covertly,privately,confidentially,furtively,clandestinely,surreptitiously,stealthily,behind closed doors,in secret,on the quiet,quietly
r|quietly,silently,softly,noiselessly,in a whisper,inaudibly,calmly,peacefully,gently,discreetly,unobtrusively,in silence,mutely
r|loudly,noisily,at the top of one's voice,deafeningly,boisterously,vociferously,aloud,clamorously,lustily,forcefully,blaringly
r|gently,softly,tenderly,lightly,kindly,mildly,carefully,delicately,moderately,gradually,smoothly,calmly,quietly
r|well-known,famous,noted,popular,notorious
r|highly,extremely,very,greatly,immensely,tremendously,exceptionally,favourably,well,decidedly,enormously,hugely,vastly,intensely
r|deeply,profoundly,intensely,greatly,seriously,thoroughly,extremely,strongly,sincerely,gravely,heartily,keenly,far down
r|widely,broadly,extensively,generally,commonly,largely,universally,far and wide,greatly,considerably,over a wide area,at large
r|closely,carefully,intently,attentively,tightly,nearby,rigorously,minutely,thoroughly,intimately,in detail,narrowly,near
r|strongly,firmly,powerfully,forcefully,vigorously,intensely,solidly,sturdily,deeply,fiercely,passionately,staunchly,robustly
r|fully,completely,entirely,wholly,totally,thoroughly,altogether,in every respect,utterly,quite,at least,perfectly,amply,in detail
r|really,genuinely,well
r|largely,mainly,mostly,chiefly,principally,to a great extent,generally,predominantly,widely,by and large,on the whole,primarily
r|increasingly,more and more,progressively,ever more,steadily,to a greater degree,growingly,gradually,with growing frequency
r|significantly,considerably,notably,markedly,substantially,appreciably,importantly,meaningfully,materially,greatly,remarkably,seriously
r|substantially,considerably,significantly,greatly,markedly,largely,materially,appreciably,essentially,extensively,mostly,solidly,sturdily
r|relatively,comparatively,fairly,rather,somewhat,quite,reasonably,moderately,to some degree,proportionately,in proportion,pretty
r|extremely,exceedingly,tremendously
r|effectively,efficiently,productively,successfully,in effect,virtually,in practice,for all practical purposes,really,competently,powerfully
r|specifically,particularly,expressly,explicitly,precisely,exactly,especially,in particular,namely,clearly,definitely,distinctly
r|typically,usually,normally,generally,commonly,characteristically,ordinarily,customarily,as a rule,representatively,in general
r|ultimately,eventually,finally,in the end,at last,in the long run,after all,fundamentally,basically,primarily,at bottom,lastly
r|primarily,mainly,chiefly,principally,mostly,largely,essentially,predominantly,above all,first and foremost,for the most part,in the first place
r|initially,at first,originally,to begin with,at the outset,at the start,in the beginning,first,early on,from the start,to start with
r|previously,formerly,earlier,before,once,beforehand,in the past,at one time,heretofore,hitherto,until now,then
r|subsequently,afterwards,later,next,then,thereafter,after that,following,in due course,consequently,at a later time,in the aftermath
r|simultaneously,at the same time,concurrently,together,in unison,at once,in tandem,all at once,in concert,in parallel,coincidentally
r|currently,now,presently,at this time
r|already,by now,even now,previously
r|overall,on the whole,in general,all in all,by and large,generally,altogether,in the main,broadly,largely,all things considered,in total,mostly
r|altogether,completely,totally,entirely,wholly,utterly,in total,all told,in all,on the whole,all in all,fully,thoroughly,absolutely
r|in general,generally,usually,mostly,on the whole,by and large,as a rule,normally,broadly,in the main,typically,overall,commonly
r|at least,at any rate,in any case,anyhow,anyway,at the very least,if nothing else,no less than,in any event,minimally,to say the least
r|in particular,especially,particularly,specifically,notably,above all,chiefly,expressly,mainly
r|on average,typically,usually,generally,normally,as a rule,commonly,by and large,ordinarily
r|in order to,so as to,with the aim of,to,with the intention of,so that,for the purpose of
r|in terms of,regarding,concerning,as regards,with regard to,relating to,with respect to,in relation to,about
r|regarding,concerning,about,with regard to,as regards,relating to,with reference to,pertaining to,on the subject of,in respect of,touching
r|according to,as stated by,as claimed by,as reported by,in the opinion of,by,as per,in line with,from,as said by
r|due to,because of,owing to,thanks to,on account of,as a result of,caused by,through,attributable to,by reason of
r|such as,for example,like,for instance,including,as,namely,say,in particular,e.g.
r|as well as,in addition to,along with,together with,besides,and,plus,not to mention,on top of,coupled with
r|whether,if,in case,whether or not,either,on the chance that
r|whereas,while,although,whilst,but,on the other hand,when in fact,in contrast,as against
r|while,during,as,whereas,although,at the same time as,whilst,for as long as,meanwhile,in the course of,though,time
r|until,till,up to,up until,before,to the time that,as far as,pending
r|unless,except if,if not,without,barring,save,excepting,other than if,except when,provided that not
r|regardless,anyway,nevertheless,nonetheless,anyhow,in any case,despite everything,all the same,still,in spite of everything,irrespective,notwithstanding
r|nonetheless,nevertheless,even so,however,still,yet,all the same,notwithstanding,regardless,in spite of that,be that as it may
r|nevertheless,nonetheless,however,still,yet,even so,notwithstanding,all the same,regardless,be that as it may,in spite of that
r|meanwhile,in the meantime,at the same time,for now,simultaneously,concurrently,in the interim,for the time being,during the interval,meantime
r|thereby,by that means,through that,in that way,thus,by doing so,because of that,as a result,hence,in consequence
r|therefore,hence
r|whereby,by which,through which,in accordance with which,wherewith,in which,by means of which
r|henceforth,from now on,from this time,hereafter,in the future,from this day forward,hence,onward,going forward,thenceforth,from here on
r|furthermore,further,in addition,what's more,besides,moreover,too,also,additionally,likewise,to boot,plus
!a|big,large,huge,great,enormous,vast,massive,giant,immense|small,little,tiny,minute,miniature,petite,slight,diminutive
!a|good,excellent,fine,positive,superb|bad,poor,awful,terrible,dreadful,inferior
!a|happy,glad,cheerful,joyful,delighted,content|sad,unhappy,sorrowful,gloomy,miserable,depressed
!a|fast,quick,rapid,swift,speedy,brisk|slow,sluggish,gradual,unhurried,leisurely
!a|easy,simple,effortless,straightforward|difficult,hard,tough,challenging,demanding,complicated
!a|hard,firm,solid,rigid,stiff|soft,gentle,tender,supple,fluffy
!a|important,significant,essential,vital,crucial,key|unimportant,trivial,minor,insignificant,petty
!a|new,fresh,novel,modern,recent,original|old,aged,ancient,antique,archaic,outdated
!a|beautiful,pretty,lovely,attractive,gorgeous|ugly,unattractive,unsightly,hideous,plain
!a|strong,powerful,sturdy,robust,mighty|weak,feeble,frail,fragile,flimsy,powerless
!a|rich,wealthy,affluent,prosperous|poor,needy,destitute,impoverished,penniless
!a|cheap,inexpensive,affordable,economical|expensive,costly,pricey,dear,exorbitant
!a|hot,warm,boiling,scorching|cold,chilly,cool,freezing,icy
!a|wet,damp,moist,soaked|dry,arid,parched
!a|clean,spotless,tidy,pure,fresh|dirty,filthy,grubby,soiled,messy,grimy
!a|quiet,silent,hushed,still,soft|loud,noisy,deafening,booming
!a|brave,courageous,bold,fearless|cowardly,timid,fearful,afraid,craven
!a|honest,truthful,sincere,candid|dishonest,deceitful,untruthful,lying,insincere
!a|kind,caring,generous,considerate|cruel,harsh,unkind,heartless,callous
!a|polite,courteous,respectful|rude,impolite,discourteous,disrespectful
!a|true,correct,accurate,right,real|false,wrong,incorrect,inaccurate,fake
!a|certain,sure,definite,confident|uncertain,unsure,doubtful,hesitant
!a|possible,feasible,likely|impossible,unlikely,unfeasible
!a|clear,obvious,evident,plain|unclear,vague,obscure,ambiguous,hazy
!a|complete,whole,entire,full|incomplete,partial,unfinished,fragmentary
!a|full,filled,packed,crowded|empty,vacant,bare,hollow
!a|careful,cautious,wary,attentive|careless,sloppy,negligent,reckless,heedless
!a|busy,occupied,active,engaged|idle,lazy,inactive,unoccupied
!a|friendly,amiable,sociable,warm|hostile,unfriendly,antagonistic,cold
!a|common,usual,ordinary,typical,normal|rare,unusual,uncommon,exceptional,scarce
!a|similar,alike,comparable|different,distinct,dissimilar,unlike,contrasting
!a|same,identical,equal|different,unequal,distinct
!a|many,numerous,plentiful,abundant|few,scarce,sparse,limited
!a|serious,grave,solemn,earnest|frivolous,light-hearted,playful,flippant
!a|safe,secure,protected,harmless|dangerous,risky,hazardous,perilous,unsafe
!a|useful,helpful,practical,valuable|useless,pointless,worthless,futile
!a|successful,prosperous,thriving,victorious|unsuccessful,failing,futile,fruitless
!a|public,communal,open,shared|private,personal,confidential,secret
!a|general,broad,universal,common|specific,particular,individual,precise
!a|short,brief,concise|long,lengthy,extended,prolonged
!a|tall,high,lofty,towering|short,low,squat,stunted
!a|wide,broad,expansive|narrow,thin,slim,tight
!a|thin,slim,slender,lean,skinny|fat,plump,stout,overweight,chubby,obese
!a|heavy,weighty,hefty,massive|light,weightless,airy,feathery
!a|bright,brilliant,luminous,radiant|dark,dim,gloomy,murky,dusky
!a|interesting,fascinating,engaging,absorbing|boring,tedious,dull,monotonous,dreary
!a|simple,plain,basic,elementary|complex,complicated,intricate,elaborate,convoluted
!a|flexible,adaptable,pliable,supple|rigid,stiff,inflexible,strict,unyielding
!a|fair,just,impartial,objective|unfair,unjust,biased,partial,prejudiced
!a|legal,lawful,legitimate,permitted|illegal,unlawful,illicit,banned,forbidden,prohibited
!a|free,independent,liberated|dependent,restricted,constrained,confined
!a|permanent,lasting,enduring,eternal|temporary,brief,fleeting,transient,short-term
!a|stable,steady,secure,firm|unstable,unsteady,shaky,insecure,volatile
!a|reliable,dependable,trustworthy|unreliable,undependable,untrustworthy,fickle
!a|generous,giving,charitable,lavish|selfish,greedy,stingy,mean,miserly
!a|healthy,well,fit,robust|ill,sick,unwell,ailing,diseased
!a|alive,living,live|dead,deceased,lifeless,extinct
!a|natural,organic,native,innate|artificial,synthetic,man-made,fake,unnatural
!a|local,regional,nearby|foreign,overseas,distant,remote
!a|formal,official,ceremonial|informal,casual,relaxed,unofficial
!a|relevant,pertinent,applicable|irrelevant,unrelated,immaterial,beside the point
!a|appropriate,suitable,proper,fitting|inappropriate,unsuitable,improper,unfitting
!a|acceptable,satisfactory,adequate|unacceptable,unsatisfactory,inadequate,insufficient
!a|logical,rational,reasonable,sensible|illogical,irrational,unreasonable,absurd
!a|eager,keen,enthusiastic|reluctant,unwilling,apathetic,indifferent,hesitant
!a|confident,assured,self-assured|insecure,doubtful,hesitant,timid
!a|calm,peaceful,tranquil,serene|agitated,anxious,restless,troubled,excited
!a|near,close,nearby,adjacent|far,distant,remote,faraway
!a|outer,outside,exterior,external|inner,inside,interior,internal
!a|upper,higher,top|lower,bottom,under
!a|first,initial,opening,earliest|last,final,closing,concluding
!a|previous,earlier,prior,former|next,following,subsequent,later,succeeding
!a|major,main,principal,chief|minor,lesser,secondary,subordinate
!a|abundant,plentiful,ample,copious|scarce,sparse,meagre,insufficient
!a|ancient,old,antique|modern,contemporary,recent,new
!a|deep,profound,bottomless|shallow,superficial,slight
!a|smooth,even,flat,sleek|rough,coarse,uneven,bumpy
!a|sharp,keen,pointed|blunt,dull,rounded
!a|straight,direct|crooked,bent,twisted,curved
!a|thick,dense,heavy|thin,sparse,fine
!v|begin,start,commence,open,launch|end,finish,conclude,stop,terminate,complete
!v|increase,rise,grow,expand,raise,boost|decrease,reduce,lessen,decline,diminish,shrink
!v|improve,enhance,better,upgrade|worsen,deteriorate,decline,degrade
!v|accept,receive,agree to,welcome|reject,refuse,decline,turn down,spurn
!v|agree,concur,consent,assent|disagree,differ,dispute,object,dissent
!v|win,triumph,succeed,prevail|lose,fail,forfeit
!v|build,construct,erect,assemble,make|destroy,demolish,ruin,wreck,raze
!v|open,unlock,unfasten,uncover|close,shut,seal,lock,fasten
!v|add,append,attach,include,insert|remove,delete,subtract,take away,erase
!v|join,connect,link,unite,combine|separate,divide,split,detach,disconnect
!v|buy,purchase,acquire,obtain|sell,trade,market,retail,vend
!v|give,provide,supply,offer,donate|take,receive,accept,grab,seize
!v|arrive,come,reach|leave,depart,go,exit,withdraw
!v|enter,go in,come in|exit,go out,leave,depart
!v|rise,climb,ascend,go up,mount|fall,drop,descend,sink,tumble
!v|remember,recall,recollect|forget,overlook,neglect
!v|love,adore,cherish,like|hate,detest,loathe,despise,dislike
!v|praise,compliment,commend,applaud|criticise,condemn,blame,censure,denounce
!v|encourage,inspire,motivate,support|discourage,deter,dissuade,dishearten
!v|permit,allow,let,authorise|forbid,ban,prohibit,prevent,disallow
!v|include,contain,incorporate|exclude,omit,leave out,bar
!v|expand,enlarge,extend,widen|contract,shrink,narrow,condense
!v|ask,enquire,question,query|answer,reply,respond
!v|hide,conceal,cover,mask,disguise|reveal,show,expose,display,uncover
!v|simplify,clarify|complicate,confuse
!v|support,back,uphold,endorse|oppose,resist,contest,challenge
!v|continue,carry on,persist,proceed|stop,cease,halt,discontinue,quit
!v|create,make,produce,form|destroy,demolish,ruin,end
!v|succeed,triumph,prosper|fail,flop,founder,collapse
!v|appear,emerge,surface,arrive|disappear,vanish,fade,recede
!v|save,store,preserve,conserve|waste,squander,spend,fritter
!v|demonstrate,prove,show,establish|disprove,refute,rebut,contradict
!v|protect,defend,guard,shield|attack,assault,raid,strike,endanger
!v|strengthen,reinforce,fortify,boost|weaken,undermine,sap,erode,impair
!n|advantage,benefit,asset,plus,gain|disadvantage,drawback,handicap,downside,weakness
!n|success,triumph,victory,achievement|failure,defeat,collapse,flop,fiasco
!n|beginning,start,opening,origin,outset|end,finish,conclusion,close,ending,termination
!n|increase,rise,growth,gain,boost|decrease,reduction,decline,fall,drop,cut
!n|truth,fact,reality|lie,falsehood,untruth,fiction,fabrication
!n|friend,companion,ally,pal|enemy,foe,opponent,adversary,rival
!n|war,conflict,fighting,combat|peace,harmony,truce,calm,ceasefire
!n|love,affection,fondness,devotion|hate,hatred,loathing,hostility,animosity
!n|joy,happiness,delight,pleasure|sorrow,sadness,grief,misery,woe
!n|strength,power,might,force|weakness,frailty,feebleness,vulnerability
!n|knowledge,understanding,awareness|ignorance,unawareness,naivety
!n|agreement,accord,harmony,consent|disagreement,dispute,conflict,discord,dissent,quarrel
!n|freedom,liberty,independence|captivity,slavery,bondage,restriction
!n|majority,most,bulk|minority,few,smaller part
!n|top,peak,summit,apex|bottom,base,foot,floor,depths
!n|front,fore,face|back,rear,tail,end
!n|inside,interior,core|outside,exterior,surface
!n|difference,contrast,disparity,variation|similarity,likeness,resemblance,sameness
!n|abundance,plenty,profusion|shortage,lack,scarcity,dearth,deficit
!n|beauty,loveliness,attractiveness|ugliness,unattractiveness,plainness
!n|health,fitness,well-being|illness,disease,sickness,ailment
!n|life,existence,being|death,dying,demise,end
!n|certainty,sureness,confidence,assurance|doubt,uncertainty,hesitation,misgiving
!n|courage,bravery,nerve,valour|cowardice,fear,timidity
!n|honour,respect,esteem,glory|shame,disgrace,dishonour,humiliation
!n|question,query,enquiry|answer,reply,response,solution
!n|problem,difficulty,trouble|solution,answer,remedy,fix
!r|quickly,fast,rapidly,swiftly,speedily|slowly,gradually,leisurely,unhurriedly
!r|always,constantly,continually,invariably,forever|never,rarely,seldom
!r|often,frequently,regularly,repeatedly|rarely,seldom,infrequently
!r|early,ahead of time,in good time|late,belatedly,tardily,behind schedule
!r|before,previously,earlier,beforehand|after,later,afterwards,subsequently
!r|together,jointly,collectively,as one|separately,individually,apart,independently
!r|completely,totally,entirely,fully,wholly|partly,partially,slightly,in part
!r|certainly,definitely,surely,undoubtedly|possibly,perhaps,maybe,doubtfully
!r|inside,indoors,within|outside,outdoors,externally
!r|above,overhead,up|below,beneath,under,underneath
!r|near,nearby,close|far,distant,afar,remotely
!r|fortunately,luckily,happily,thankfully|unfortunately,sadly,regrettably,unluckily
!r|openly,publicly,frankly,overtly|secretly,covertly,privately,furtively,clandestinely
!r|loudly,noisily,aloud|quietly,silently,softly,noiselessly
!r|well,effectively,skilfully,ably|badly,poorly,ineptly,inadequately
!r|carefully,cautiously,meticulously,attentively|carelessly,recklessly,thoughtlessly,sloppily
!r|easily,effortlessly,readily,smoothly|with difficulty,painfully,laboriously,arduously
`;
